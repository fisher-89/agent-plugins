import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  ActiveRunView,
  ArtifactDescriptor,
  ArtifactEnvelope,
  ChangeDetail,
} from '../../../types/dto';
import { useChangeDetail, type ChangeDetailState } from './use-change-detail';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

function fakeDetail(artifacts: ArtifactDescriptor[]): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: '2026-09-01',
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
            executorSessionId: null,
            evaluatorSessionId: null,
            decisionSessionId: null,
          },
        ],
      },
    ],
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts,
  };
}

const twoArtifacts: ArtifactDescriptor[] = [
  { kind: 'tasks-progress', source: 'tasks.md', title: '任务进度', phase: 'dev-design' },
  { kind: 'markdown-doc', source: 'proposal.md', title: '提案', phase: 'proposal' },
];

/** 统一视图应答包装（get_change_detail 线面 = { detail, activeRun }——机械
 * 随动：既有裸 detail 应答经本 helper 出线）。 */
function unified(d: ChangeDetail | null, activeRun: ActiveRunView | null = null) {
  return { detail: d, activeRun };
}

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
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
      root: '/repo',
      id: 'add-feature',
    });
    expect(result.current.loading).toBe(false);
  });

  it('详情返回产物清单后同一周期内逐个 invoke("read_artifact") 并组装信封', async () => {
    // 按命令名 + 参数精确分发
    invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail(twoArtifacts)));
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
      id: 'add-feature',
      kind: 'tasks-progress',
      source: 'tasks.md',
    });
    expect(readCalls[1][1]).toEqual({
      root: '/repo',
      id: 'add-feature',
      kind: 'markdown-doc',
      source: 'proposal.md',
    });
    expect(result.current.artifacts[0].kind).toBe('tasks-progress');
    expect(result.current.artifacts[1].fallbackText).toBe('保底：提案');
  });

  it('单个 read_artifact reject 时该产物以 Fallback 形态呈现，其余正常组装', async () => {
    invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail(twoArtifacts)));
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

  it('id 为 null 时不发起任何 invoke（空身份早退）', () => {
    renderHook(() => useChangeDetail('/repo', null));
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('产物清单为空数组时 artifacts 为空数组且无额外 invoke', async () => {
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'empty'));

    await waitFor(() => expect(result.current.detail).not.toBeNull());
    expect(result.current.artifacts).toEqual([]);
    expect(invokeMock.mock.calls.every(([name]) => name === 'get_change_detail')).toBe(true);
  });

  it('除显式刷新外无轮询：推进虚拟计时后 invoke 次数不增长', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
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

  afterEach(() => {
    vi.useRealTimers();
  });

  it('挂载后请求未返回前 loading 置位且 artifacts 初始为空数组', () => {
    invokeMock.mockImplementation(() => new Promise<unknown>(() => {}));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    expect(result.current.loading).toBe(true);
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.detail).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('换 id 首轮重新置位 loading（首载键随 root / id 重置）', async () => {
    let callCount = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command !== 'get_change_detail') return Promise.resolve(null);
      callCount += 1;
      if (callCount === 1) return Promise.resolve(unified(fakeDetail([])));
      return new Promise<unknown>(() => {});
    });
    const { result, rerender } = renderHook<ChangeDetailState, { id: string }>(
      (props) => useChangeDetail('/repo', props.id),
      { initialProps: { id: 'change-a' } },
    );
    await act(async () => {});
    expect(result.current.loading).toBe(false);

    rerender({ id: 'change-b' });
    await act(async () => {});
    expect(result.current.loading).toBe(true);
  });

  it('通知触发的自动重查挂起期间 loading 保持 false（过程中自动刷新不阻塞加载态）', async () => {
    vi.useFakeTimers();
    let roundCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command !== 'get_change_detail') return Promise.resolve(null);
      roundCalls += 1;
      if (roundCalls === 1) return Promise.resolve(unified(fakeDetail([])));
      return new Promise<unknown>(() => {});
    });
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(result.current.detail).not.toBeNull();
    expect(result.current.loading).toBe(false);

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(roundCalls).toBe(2);
    expect(result.current.loading).toBe(false);
  });

  it('read_artifact 返回 null（非 reject）时该产物降级为 Fallback 信封', async () => {
    const descriptor: ArtifactDescriptor = {
      kind: 'markdown-doc',
      source: 'proposal.md',
      title: '提案',
      phase: 'proposal',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail([descriptor])));
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

  it('id 切回 null 时重置 detail / artifacts / loading / error', async () => {
    const descriptor: ArtifactDescriptor = {
      kind: 'tasks-progress',
      source: 'tasks.md',
      title: '任务进度',
      phase: 'dev-design',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail([descriptor])));
      }
      return Promise.resolve(envelopeFor(descriptor));
    });
    const { result, rerender } = renderHook<ChangeDetailState, { id: string | null }>(
      (props) => useChangeDetail('/repo', props.id),
      { initialProps: { id: 'add-feature' } },
    );
    await waitFor(() => expect(result.current.artifacts).toHaveLength(1));

    rerender({ id: null });
    await waitFor(() => expect(result.current.artifacts).toEqual([]));
    expect(result.current.detail).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('显式 refresh 触发重新拉取详情', async () => {
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
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
    let resolveFirst: ((value: unknown) => void) | null = null;
    let callCount = 0;
    invokeMock.mockImplementation(() => {
      callCount += 1;
      if (callCount === 1) {
        return new Promise<unknown>((resolve) => {
          resolveFirst = resolve;
        });
      }
      // 第二次刷新后的请求永不返回，用于观察旧响应是否被正确丢弃
      return new Promise<unknown>(() => {});
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
      resolveFirst?.(unified(fakeDetail([])));
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
      phase: 'dev-design',
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        detailCallCount += 1;
        if (detailCallCount === 1) {
          return Promise.resolve(unified(fakeDetail([descriptor])));
        }
        return new Promise<unknown>(() => {});
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
      return new Promise<unknown>(() => {});
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

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：get_change_detail / read_artifact 裸
// invoke → typed bindings 机械替换后，invoke 命令名与参数逐字不变（生成绑定
// 底层仍走同模块 invoke，mock 机制切换后依旧生效）。
// ---------------------------------------------------------------------------

describe('useChangeDetail：生成绑定调用面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('经生成绑定入口后 invoke 收到 "get_change_detail" 与 "read_artifact"，详情 + 产物信封同周期组装透传不变', async () => {
    invokeMock.mockImplementation((command: string, params: { kind?: string; source?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail(twoArtifacts)));
      }
      const descriptor = twoArtifacts.find(
        (d) => d.kind === params.kind && d.source === params.source,
      );
      return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
    });
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.artifacts).toHaveLength(2));
    expect(invokeMock).toHaveBeenCalledWith('get_change_detail', {
      root: '/repo',
      id: 'add-feature',
    });
    expect(invokeMock).toHaveBeenCalledWith('read_artifact', {
      root: '/repo',
      id: 'add-feature',
      kind: 'tasks-progress',
      source: 'tasks.md',
    });
    expect(result.current.detail?.name).toBe('add-feature');
    expect(result.current.artifacts[1].fallbackText).toBe('保底：提案');
  });

  it('detail 为 null / 产物清单空数组 → 零后续调用保持；read_artifact 返回 null 降级 Fallback 信封不变', async () => {
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'empty'));
    await waitFor(() => expect(result.current.detail).not.toBeNull());

    expect(result.current.artifacts).toEqual([]);
    expect(invokeMock.mock.calls.every(([name]) => name === 'get_change_detail')).toBe(true);

    const descriptor: ArtifactDescriptor = {
      kind: 'markdown-doc',
      source: 'proposal.md',
      title: '提案',
      phase: 'proposal',
    };
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail([descriptor])));
      }
      return Promise.resolve(null);
    });
    const fallbackCase = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await waitFor(() => expect(fallbackCase.result.current.artifacts).toHaveLength(1));
    expect(fallbackCase.result.current.artifacts[0].version).toBe(0);
    expect(fallbackCase.result.current.artifacts[0].fallbackText).toContain('产物读取失败');
  });

  it('单个 read_artifact reject → 该产物 Fallback 其余正常；get_change_detail reject → 错误态清空数据不变', async () => {
    invokeMock.mockImplementation((command: string, params: { kind?: string }) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail(twoArtifacts)));
      }
      if (params.kind === 'tasks-progress') {
        return Promise.reject(new Error('读取失败'));
      }
      const descriptor = twoArtifacts.find((d) => d.kind === params.kind);
      return Promise.resolve(descriptor ? envelopeFor(descriptor) : null);
    });
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await waitFor(() => expect(result.current.artifacts).toHaveLength(2));

    expect(result.current.artifacts[0].fallbackText).toContain('产物读取失败');
    expect(result.current.artifacts[1].version).toBe(1);
    expect(result.current.error).toBeNull();

    invokeMock.mockReset();
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const failed = renderHook(() => useChangeDetail('/repo', 'broken'));
    await waitFor(() => expect(failed.result.current.loading).toBe(false));

    expect(failed.result.current.error).toContain('IPC 断开');
    expect(failed.result.current.detail).toBeNull();
    expect(failed.result.current.artifacts).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 统一视图取数与通知去抖（unify-run-state-persistence D8 / R7）：
// activeRun 双态就位 + notifyRefresh 300ms / notifyTranscript 150ms 尾随去抖
// ---------------------------------------------------------------------------

/** 在飞 run 活面 fixture（ActiveRunView 线面；steps 全词汇 emit 序）。 */
function activeRun(): ActiveRunView {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 1,
    ask: null,
    archive: null,
    startedAt: '2026-10-09T08:00:00Z',
    nodes: [
      {
        phase: 'implement',
        attempt: 1,
        step: 'executor',
        status: 'passed',
        sessionId: 'ses-1',
        detail: null,
        instanceNo: 0,
        outputRef: null,
        startedAt: 1_727_000_000_000,
        endedAt: null,
        tokenInput: null,
        tokenOutput: null,
      },
      {
        phase: 'implement',
        attempt: 1,
        step: 'staticCheck',
        status: 'running',
        sessionId: null,
        detail: null,
        instanceNo: 0,
        outputRef: null,
        startedAt: 1_727_000_000_000,
        endedAt: null,
        tokenInput: null,
        tokenOutput: null,
      },
    ],
    edges: [],
  };
}

describe('useChangeDetail：统一视图取数与通知去抖（D8）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('getChangeDetail 返回 ChangeDetailUnified → detail 与 activeRun 双态就位（AC-7 前端半边）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_change_detail') {
        return Promise.resolve(unified(fakeDetail([]), activeRun()));
      }
      return Promise.resolve(null);
    });
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));

    await waitFor(() => expect(result.current.activeRun).not.toBeNull());
    expect(result.current.detail).not.toBeNull();
    expect(result.current.activeRun?.runId).toBe('run-1727');
    expect(result.current.activeRun?.status).toBe('running');
    expect(result.current.activeRun?.nodes).toHaveLength(2);
    expect(result.current.activeRun?.nodes[1].step).toBe('staticCheck');
  });

  it('notifyRefresh 去抖：窗内连续多次 → 恰一次重查（fake timers 前进 300ms 后一拍）', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    const base = invokeMock.mock.calls.length;

    act(() => {
      result.current.notifyRefresh();
      result.current.notifyRefresh();
      result.current.notifyRefresh();
    });
    act(() => {
      vi.advanceTimersByTime(299);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '去抖窗内零重查').toBe(base);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '300ms 到点恰一次重查').toBe(base + 1);
  });

  it('notifyRefresh 尾随语义：窗内新到达重置计时（最后一拍必达——重查在最后一次通知后 300ms）', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    const base = invokeMock.mock.calls.length;

    act(() => {
      result.current.notifyRefresh();
    });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await act(async () => {});
    act(() => {
      result.current.notifyRefresh();
    });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '首拍计时被重置（299ms 处静默）').toBe(base);

    act(() => {
      vi.advanceTimersByTime(100);
    });
    await act(async () => {});
    expect(
      invokeMock.mock.calls.length,
      '重查发生在最后一次通知后 300ms（trailing 最后一拍必达）',
    ).toBe(base + 1);
  });

  it('通知归一：会话事件与其余同走 notifyRefresh（300ms 尾随去抖），refreshTick 一拍照常自增', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    const base = invokeMock.mock.calls.length;
    const tickBase = result.current.refreshTick;

    act(() => {
      result.current.notifyRefresh();
    });
    act(() => {
      vi.advanceTimersByTime(299);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '去抖窗内静默（尾随最后一拍必达）').toBe(base);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '通知到点重查一拍').toBe(base + 1);
    expect(result.current.refreshTick, '统一 tick 自增（会话组件据此重评估）').toBe(tickBase + 1);
  });

  it('卸载弃挂起计时器：notifyRefresh 后卸载 → 去抖窗过点零泄漏重查', async () => {
    vi.useFakeTimers();
    invokeMock.mockResolvedValue(unified(fakeDetail([])));
    const { result, unmount } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    const base = invokeMock.mock.calls.length;

    act(() => {
      result.current.notifyRefresh();
    });
    unmount();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '卸载弃挂起计时器（零泄漏重查）').toBe(base);
  });
});

// ---------------------------------------------------------------------------
// 失败轮降级与有界重试（fix-flow-refresh-degradation AC-1–AC-4 / AC-7）：
// get_change_detail reject 且 detail 在手 → 保 last-good（detail / activeRun /
// artifacts 不清）+ 内联失败态 refreshFailed + 退避 1s / 2s / 4s 至多 3 次自动
// 补发；新意图（手动刷新 / 新通知 / 换 root / id / 卸载）取消挂起重试并重置退
// 避序列；transcriptTick 不入取消通道；首载失败维持整页 error 兜底（不重试）。
// 计时面经 vi.useFakeTimers + advanceTimersByTime 逐拍推进（Mock策略表）。
// ---------------------------------------------------------------------------

/** get_change_detail 单轮应答（轮脚本最小单元——按调用序逐轮消费）。 */
type DetailRound = () => Promise<unknown>;

/** 成功轮（统一视图应答；缺省空 runs / 空 activeRun）。 */
function okRound(d: ChangeDetail = fakeDetail([]), run: ActiveRunView | null = null): DetailRound {
  return () => Promise.resolve(unified(d, run));
}

/** 失败轮（错误串经 String(err) 落 refreshFailed——含后端串原样）。 */
function rejectRound(message: string): DetailRound {
  return () => Promise.reject(new Error(message));
}

/** 真 None 轮（未知 id——清面兜底语义）。 */
function nullRound(): DetailRound {
  return () => Promise.resolve(null);
}

/** 轮脚本装配（Mock策略表「按命令名 + 调用序可编程」）：get_change_detail 第
 * N 轮逐轮消费（越界驻留末轮）；其余命令空态应答。返回轮计数观察面。 */
function scriptDetailRounds(rounds: DetailRound[]): () => number {
  let called = 0;
  invokeMock.mockImplementation((command: string) => {
    if (command !== 'get_change_detail') return Promise.resolve(null);
    const round = rounds[Math.min(called, rounds.length - 1)];
    called += 1;
    return round();
  });
  return () => called;
}

/** 推进虚拟时间并冲刷微任务链（fake timers 下的轮拍推进单点）。 */
async function flushTimers(ms: number): Promise<void> {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
  await act(async () => {});
}

/** 携 run 全史行的详情（重试成功轮「迁移间更新不被吃掉」的步迁移新数据面）。 */
function detailWithRuns(): ChangeDetail {
  return {
    ...fakeDetail([]),
    runs: [
      {
        runId: 'run-migrate',
        status: 'running',
        reason: null,
        startedAt: '2026-10-09T08:00:00Z',
        finishedAt: null,
        nodes: [],
        edges: [],
      },
    ],
  };
}

describe('useChangeDetail：失败轮降级保 last-good 与终态自愈（AC-1 / AC-4）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('失败轮保 last-good：第 2 轮 reject → detail / activeRun 维持上一成功轮值，refreshFailed 携后端错误串、loading 复位、error 保持 null', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([]), activeRun()),
      rejectRound('db: 注入失败：读事务不可用'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    // 首轮成功：detail / activeRun 在手、内联失败态初始 null（既有回归保留面）
    expect(result.current.detail).not.toBeNull();
    expect(result.current.activeRun).not.toBeNull();
    expect(result.current.refreshFailed).toBeNull();
    const lastGoodDetail = result.current.detail;
    const lastGoodRun = result.current.activeRun;

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);

    expect(roundCalls()).toBe(2);
    expect(result.current.refreshFailed).toContain('db: 注入失败：读事务不可用');
    // 保 last-good：上一成功轮值引用不变（detail / activeRun 不清零）
    expect(result.current.detail).toBe(lastGoodDetail);
    expect(result.current.activeRun).toBe(lastGoodRun);
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('失败后第 1 次重试成功（应答含步迁移新数据）→ refreshFailed 清 null、detail / runs 更新到最新轮值（迁移间更新不被吃掉）', async () => {
    vi.useFakeTimers();
    scriptDetailRounds([okRound(fakeDetail([])), rejectRound('首败'), okRound(detailWithRuns())]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(result.current.detail?.runs).toHaveLength(0);

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(result.current.refreshFailed).not.toBeNull();

    await flushTimers(1000); // 第 1 次退避重试（1s）

    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.detail?.runs).toHaveLength(1);
    expect(result.current.detail?.runs[0].runId).toBe('run-migrate');
    expect(result.current.loading).toBe(false);
  });

  it('失败 → 重试发起 挂起期间 refreshFailed 保持（轮启动 patch 不清提示——D3 防闪烁）；成功轮才清 null', async () => {
    vi.useFakeTimers();
    let release!: (value: unknown) => void;
    scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('首败'),
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
      okRound(detailWithRuns()),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    const stale = result.current.refreshFailed;
    expect(stale).not.toBeNull();

    // 1s 重试发起并挂起（轮启动不清 refreshFailed）
    await flushTimers(1000);
    expect(result.current.loading).toBe(false);
    expect(result.current.refreshFailed).toBe(stale);

    // 挂起轮以成功落定：提示才消隐
    await act(async () => {
      release(unified(detailWithRuns()));
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.detail?.runs).toHaveLength(1);
  });

  it('首载失败（detail 不在手）→ error 置位整页兜底态、refreshFailed 保持 null、推进 8s+ 零自动补发（首载失败不调度重试）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([rejectRound('IPC 断开')]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'broken'));
    await act(async () => {});

    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.detail).toBeNull();
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.loading).toBe(false);

    await flushTimers(8_000);
    expect(roundCalls()).toBe(1);
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.error).toContain('IPC 断开');
  });

  it('连续失败（首败 + 3 次重试全败）→ 每轮 refreshFailed 更新为最新错误串；终轮后仍 last-good（detail 不清）', async () => {
    vi.useFakeTimers();
    scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('失败 A'),
      rejectRound('失败 B'),
      rejectRound('失败 C'),
      rejectRound('失败 D'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    const lastGoodDetail = result.current.detail;

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(result.current.refreshFailed).toContain('失败 A');

    await flushTimers(1000);
    expect(result.current.refreshFailed).toContain('失败 B');
    await flushTimers(2000);
    expect(result.current.refreshFailed).toContain('失败 C');
    await flushTimers(4000);
    expect(result.current.refreshFailed).toContain('失败 D');

    expect(result.current.detail).toBe(lastGoodDetail);
    expect(result.current.error).toBeNull();
  });

  it('真 None：失败轮后重试成功返回 null → 清面（detail / activeRun / artifacts 置空）且 refreshFailed 清 null（None 与 Err 分流不混同）', async () => {
    vi.useFakeTimers();
    scriptDetailRounds([okRound(fakeDetail([]), activeRun()), rejectRound('首败'), nullRound()]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(result.current.detail).not.toBeNull();
    expect(result.current.activeRun).not.toBeNull();

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(result.current.refreshFailed).not.toBeNull();

    await flushTimers(1000);

    expect(result.current.detail).toBeNull();
    expect(result.current.activeRun).toBeNull();
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('终态竞态洞自愈（AC-4）：首轮成功 → 通知重查 reject（Finished 后无下一条通知）→ 1s 重试成功返回终态数据 → 终局自愈不悬、全程 last-good 不塌', async () => {
    vi.useFakeTimers();
    scriptDetailRounds([
      okRound(fakeDetail([]), activeRun()),
      rejectRound('终态重查失败'),
      okRound(detailWithRuns(), null),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(result.current.activeRun).not.toBeNull();

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    // 终态重查失败：last-good 不塌（activeRun 运行中形态保持 + 横幅在）
    expect(result.current.activeRun).not.toBeNull();
    expect(result.current.refreshFailed).toContain('终态重查失败');

    await flushTimers(1000);

    // 终局自愈：终态数据就位（activeRun 除名 → null）、提示消隐、零悬置
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.activeRun).toBeNull();
    expect(result.current.detail?.runs).toHaveLength(1);
    expect(result.current.loading).toBe(false);
  });
});

describe('useChangeDetail：有界重试节奏与取消通道（AC-2 / AC-3）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('退避节奏（fake timers）：首败后恰 1s / 2s / 4s 逐拍自动补发，任一退避点前零提前触发（自动补发至多 3 次）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('首败'),
      rejectRound('补发 1'),
      rejectRound('补发 2'),
      rejectRound('补发 3'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    expect(roundCalls()).toBe(1);

    // 通知轮（首败）：300ms 去抖后发起
    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(299);
    expect(roundCalls()).toBe(1);
    await flushTimers(1);
    expect(roundCalls()).toBe(2);

    // 退避 1s：恰 1s 补发（999ms 处静默）
    await flushTimers(999);
    expect(roundCalls()).toBe(2);
    await flushTimers(1);
    expect(roundCalls()).toBe(3);

    // 退避 2s：恰 2s 补发（1999ms 处静默）
    await flushTimers(1999);
    expect(roundCalls()).toBe(3);
    await flushTimers(1);
    expect(roundCalls()).toBe(4);

    // 退避 4s：恰 4s 补发（3999ms 处静默）——自动补发累计 3 次
    await flushTimers(3999);
    expect(roundCalls()).toBe(4);
    await flushTimers(1);
    expect(roundCalls()).toBe(5);
    expect(result.current.refreshFailed).toContain('补发 3');
  });

  it('穷尽静默：3 次重试穷尽后推进超长时间（> 4s）→ 无第 4 次自动查询、refreshFailed 保持（等待通知 / 显式刷新兜底）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('首败'),
      rejectRound('补发 1'),
      rejectRound('补发 2'),
      rejectRound('补发 3'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    await flushTimers(1000);
    await flushTimers(2000);
    await flushTimers(4000);
    expect(roundCalls()).toBe(5);

    await flushTimers(60_000);
    expect(roundCalls()).toBe(5);
    expect(result.current.refreshFailed).toContain('补发 3');
    expect(result.current.detail).not.toBeNull();
  });

  it('手动刷新取消：1s 重试挂起期间 refresh() → 旧重试作废、以手动刷新轮为准；其后新失败从 1s 重新起算（退避序列重置）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('通知轮失败'),
      rejectRound('手动轮失败'),
      rejectRound('手动轮重试失败'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(roundCalls()).toBe(2);
    expect(result.current.refreshFailed).toContain('通知轮失败');

    // 手动刷新：作废挂起重试 + 立即发起新轮
    act(() => {
      result.current.refresh();
    });
    await act(async () => {});
    expect(roundCalls()).toBe(3);
    expect(result.current.refreshFailed).toContain('手动轮失败');

    // 手动轮失败 +1s：退避序列重置后按 1s 补发（未取消的旧重试若残留将在此
    // 双发——计数恰 4 即取消语义与零交错的复合证据）
    await flushTimers(1000);
    expect(roundCalls()).toBe(4);
    expect(result.current.refreshFailed).toContain('手动轮重试失败');
  });

  it('新通知取消：重试挂起期间 notifyRefresh() → 旧重试作废、退避重置；旧重试点过境零补发（全程无交错重复查询）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('通知轮失败'),
      rejectRound('新通知轮失败'),
      rejectRound('新通知轮重试失败'),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(roundCalls()).toBe(2);

    // 重试挂起期间（旧退避点 = t0+1000 未到）新通知到达
    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    // 新通知轮经 300ms 去抖发起（call 3）
    expect(roundCalls()).toBe(3);
    expect(result.current.refreshFailed).toContain('新通知轮失败');

    // 旧重试点（首败 +1s = t+1300）过境 → 零补发（旧重试已作废；若残留将在此
    // 交错触发第 4 次查询）
    await flushTimers(700);
    expect(roundCalls()).toBe(3);

    // 新通知轮失败 +1s（t+1600）：退避序列重置后按 1s 补发
    await flushTimers(300);
    expect(roundCalls()).toBe(4);
    expect(result.current.refreshFailed).toContain('新通知轮重试失败');
  });

  it('换 id 作废挂起重试：rerender 变更 id → 新 id 轮照常发起、原退避时间推进零补发（零交错）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('id-a 通知轮失败'),
      okRound(fakeDetail([])),
    ]);
    const { result, rerender } = renderHook<ChangeDetailState, { id: string }>(
      (props) => useChangeDetail('/repo', props.id),
      { initialProps: { id: 'change-a' } },
    );
    await act(async () => {});
    expect(roundCalls()).toBe(1);

    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(roundCalls()).toBe(2);
    expect(result.current.refreshFailed).toContain('id-a 通知轮失败');

    // 换 id：挂起重试作废 + 新 id 轮立即发起
    rerender({ id: 'change-b' });
    await act(async () => {});
    expect(roundCalls()).toBe(3);
    expect(invokeMock).toHaveBeenLastCalledWith('get_change_detail', {
      root: '/repo',
      id: 'change-b',
    });

    // 原退避时间窗（远超 1s）推进 → 零补发（零遗留计时器、零交错查询）
    await flushTimers(10_000);
    expect(roundCalls()).toBe(3);
  });

  it('卸载后推进时间 → 零自动补发（零遗留计时器）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('首败'),
      okRound(fakeDetail([])),
    ]);
    const { result, unmount } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});
    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(300);
    expect(roundCalls()).toBe(2);

    unmount();
    await flushTimers(10_000);
    expect(roundCalls()).toBe(2);
  });

  it('通知去抖拍不打断重试链：重试挂起期间 notifyRefresh() → 重试按原退避点补发（去抖拍前作废旧重试）', async () => {
    vi.useFakeTimers();
    const roundCalls = scriptDetailRounds([
      okRound(fakeDetail([])),
      rejectRound('首败'),
      okRound(detailWithRuns()),
    ]);
    const { result } = renderHook(() => useChangeDetail('/repo', 'add-feature'));
    await act(async () => {});

    act(() => {
      result.current.refresh();
    });
    await flushTimers(300);
    expect(roundCalls()).toBe(2);
    expect(result.current.refreshFailed).not.toBeNull();

    // 通知到达：300ms 去抖窗内到达即重置计时，最后一拍补发重查
    act(() => {
      result.current.notifyRefresh();
    });
    await flushTimers(150);
    expect(roundCalls(), '去抖窗内零重查').toBe(2);

    await flushTimers(150);
    expect(roundCalls(), '通知到点补发重查一拍').toBe(3);
    expect(result.current.refreshFailed).toBeNull();
    expect(result.current.detail?.runs).toHaveLength(1);
  });
});
