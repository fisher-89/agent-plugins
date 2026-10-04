import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary } from '../../../types/dto';
import { useAgentRunHistory } from './use-agent-run-history';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_sessions / agent_session_transcript）。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

/** workspace root（会话 id 为 core 铸造全局唯一，取数 invoke 携 root 寻址） */
const ROOT = 'C:\\demo\\beta';

function summary(id: string, updatedAt: number): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: { engine: 'sdk', model: 'glm-high', permissionMode: 'bypassPermissions' },
      provenance: { source: 'debug', sourceRef: null },
      createdAt: 1727000000000,
      updatedAt,
    },
    stats: { turnCount: 2, totalDurationMs: 500, inputTokens: null, outputTokens: null },
    turns: [],
  };
}

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 'ses-9-1727000000000',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery"}',
  };
}

const ID_LATEST = 'ses-3-1727000000000';
const ID_OLDER = 'ses-2-1727000000000';

/** 后端清单形态：updated_at 降序（ses-3 最近）。 */
const SESSIONS_DESC = [summary(ID_LATEST, 300), summary(ID_OLDER, 100)];

describe('useAgentRunHistory：显式刷新与点开重放', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('挂载即自动取数：invoke("agent_sessions") 恰一次（debug 来源过滤），sessions 承接清单', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('refresh() 重取仍携同一 root：invoke("agent_sessions") 重发且 sessions 按后端降序原样承接', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    act(() => {
      result.current.refresh();
    });
    // 第二次 invoke 发起瞬间其 .then 尚未 flush（loading 仍 true）：落定断言一并置于 waitFor 内轮询
    await waitFor(() => {
      expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(2);
      expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
        root: ROOT,
        source: 'debug',
        sourceRef: null,
      });
      expect(result.current.sessions).toEqual(SESSIONS_DESC);
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBeNull();
    });
  });

  it('openSession(id) → invoke("agent_session_transcript", { root, sessionId }) → events 更新、selectedSessionId 跟随', async () => {
    invokeMock.mockResolvedValue([]);
    const events = [runStarted(0), raw(1)];
    invokeMock.mockImplementation((command: string, params?: { sessionId?: string }) => {
      if (command === 'agent_session_transcript') {
        return Promise.resolve(params?.sessionId === ID_LATEST ? events : []);
      }
      return Promise.resolve([]);
    });
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.events).toEqual(events));

    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: ID_LATEST,
    });
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
  });

  it('refresh reject：error 置串、loading 复位（显式触发失败不静默）', async () => {
    invokeMock.mockRejectedValue('db: 清单打开失败');
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toBe('db: 清单打开失败'));

    expect(result.current.loading).toBe(false);
    expect(result.current.sessions).toEqual([]);
  });

  it('refresh 失败后重试成功：error 清回 null、sessions 承接清单（失败不黏滞）', async () => {
    invokeMock.mockRejectedValueOnce('db: 清单打开失败');
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.error).toBe('db: 清单打开失败'));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => {
      expect(result.current.sessions).toEqual(SESSIONS_DESC);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('refresh 仅重查清单轨道：已点开的重放 events 与 selectedSessionId 不被波及', async () => {
    const events = [runStarted(0)];
    invokeMock.mockImplementation((command: string, params?: { sessionId?: string }) =>
      command === 'agent_session_transcript' && params?.sessionId === ID_LATEST
        ? Promise.resolve(events)
        : Promise.resolve(SESSIONS_DESC),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.selectedSessionId).toBe(ID_LATEST));
    await waitFor(() => expect(result.current.events).toEqual(events));

    act(() => {
      result.current.refresh();
    });
    // 重查发起与 loading 复位一并轮询（第二次取数落定前 loading 仍为 true）
    await waitFor(() => {
      expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(2);
      expect(result.current.loading).toBe(false);
    });

    // 重放轨道状态保持：refresh 不清空 events / selectedSessionId
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
    expect(result.current.events).toEqual(events);
    expect(result.current.loading).toBe(false);
  });

  it('重放轨道取数中单独置 loading：点开会话未返回时 loading=true，返回后复位', async () => {
    invokeMock.mockResolvedValue([]);
    let resolveTranscript: (events: AgentEvent[]) => void = () => {};
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_session_transcript'
        ? new Promise<AgentEvent[]>((resolve) => {
            resolveTranscript = resolve;
          })
        : Promise.resolve(SESSIONS_DESC),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    await act(async () => {
      resolveTranscript([runStarted(0)]);
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
  });

  it('openSession reject：error 置串、loading 复位、sessions 清单不受影响', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    invokeMock.mockRejectedValue('db: 转录读取失败');
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.error).toBe('db: 转录读取失败'));

    expect(result.current.loading).toBe(false);
    expect(result.current.sessions).toEqual(SESSIONS_DESC);
  });

  it('取数进行中 loading=true、返回后复位；两轨道任一取数中即 loading', async () => {
    let resolveSessions: (sessions: SessionSummary[]) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise<SessionSummary[]>((resolve) => {
          resolveSessions = resolve;
        }),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    await act(async () => {
      resolveSessions(SESSIONS_DESC);
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('清单轨道竞态守卫：切走 source 后旧查询落定不回写 sessions、不置入旧 error', async () => {
    const resolvers: Array<(sessions: SessionSummary[]) => void> = [];
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions'
        ? new Promise<SessionSummary[]>((resolve) => {
            resolvers.push(resolve);
          })
        : Promise.resolve([]),
    );
    type SourceProp = { source: 'debug' | 'change' };
    const { result, rerender } = renderHook(
      (props: SourceProp) => useAgentRunHistory(ROOT, props.source),
      { initialProps: { source: 'debug' } },
    );

    rerender({ source: 'change' });
    expect(resolvers).toHaveLength(2);

    // 新 source 先落定：sessions 承接新清单
    const fresh = [summary('ses-change-fresh', 900)];
    await act(async () => {
      resolvers[1]?.(fresh);
    });
    await waitFor(() => expect(result.current.sessions).toEqual(fresh));

    // 旧 source 后落定（reject）：不得回写 sessions、不得置入旧错误
    await act(async () => {
      resolvers[0]?.(undefined as unknown as SessionSummary[]);
    });
    await act(async () => {});
    expect(result.current.sessions).toEqual(fresh);
    expect(result.current.error).toBeNull();
  });

  it('重放轨道竞态守卫：连续点开后旧转录后到不回写 events 与 selectedSessionId', async () => {
    const transcriptResolvers: Array<(events: AgentEvent[]) => void> = [];
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_session_transcript'
        ? new Promise<AgentEvent[]>((resolve) => {
            transcriptResolvers.push(resolve);
          })
        : Promise.resolve([]),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.openSession(ID_OLDER);
    });
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    expect(transcriptResolvers).toHaveLength(2);

    const latestEvents = [raw(0)];
    await act(async () => {
      transcriptResolvers[1]?.(latestEvents);
    });
    await waitFor(() => expect(result.current.events).toEqual(latestEvents));
    expect(result.current.selectedSessionId).toBe(ID_LATEST);

    // 旧会话转录后到：不回写（cleanup 置 cancelled）
    const staleEvents = [runStarted(0)];
    await act(async () => {
      transcriptResolvers[0]?.(staleEvents);
    });
    await act(async () => {});
    expect(result.current.events).toEqual(latestEvents);
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
    expect(result.current.error).toBeNull();
  });

  it('root 为 null（未选定 workspace）：不发起任何 invoke，sessions 保持空态（跳过取数分支）', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(null, 'debug'));

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.sessions).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();

    // 重放轨道同样跳过：null root 下 openSession 不触发 invoke、events 保持空态
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.events).toEqual([]);
    expect(result.current.selectedSessionId).toBeNull();
  });

  it('agent_sessions 返回 []：sessions 为空数组不崩', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.refresh();
    });
    // loading 复位的状态更新可能晚于 sessions 到达：三断言一并置于 waitFor 内轮询
    await waitFor(() => {
      expect(result.current.sessions).toEqual([]);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('连续 openSession 不同会话：events 以最后一次为准、selectedSessionId 跟随', async () => {
    const eventsFirst = [runStarted(0)];
    const eventsSecond = [raw(0)];
    invokeMock.mockImplementation((command: string, params?: { sessionId?: string }) => {
      if (command === 'agent_session_transcript') {
        return Promise.resolve(params?.sessionId === ID_OLDER ? eventsFirst : eventsSecond);
      }
      return Promise.resolve([]);
    });
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.openSession(ID_OLDER);
    });
    await waitFor(() => expect(result.current.selectedSessionId).toBe(ID_OLDER));

    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.selectedSessionId).toBe(ID_LATEST));
    await waitFor(() => expect(result.current.events).toEqual(eventsSecond));
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：agent_sessions / agent_session_transcript
// 裸 invoke → typed bindings 机械替换后，invoke 命令名与参数逐字不变（生成
// 绑定底层仍走同模块 invoke，mock 机制切换后依旧生效）。
// ---------------------------------------------------------------------------

describe('useAgentRunHistory：生成绑定调用面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('经生成绑定入口后 invoke 收到 "agent_sessions" + { root, source, sourceRef }（挂载恰一次）与 "agent_session_transcript" + { root, sessionId }，清单降序承接与事件透传不变', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    const events = [runStarted(0)];
    invokeMock.mockImplementation((command: string, params?: { sessionId?: string }) =>
      command === 'agent_session_transcript' && params?.sessionId === ID_LATEST
        ? Promise.resolve(events)
        : Promise.resolve([]),
    );
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.events).toEqual(events));

    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
    });
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: ID_LATEST,
    });
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(1);
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
  });

  it('agent_sessions 返回 [] → sessions 为空数组不崩（绑定切换不改变空态）', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => {
      expect(result.current.sessions).toEqual([]);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('refresh / openSession reject → error 置串、loading 复位、sessions 清单不受影响（错误路径不变）', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    invokeMock.mockRejectedValue('db: 转录读取失败');
    act(() => {
      result.current.openSession(ID_LATEST);
    });
    await waitFor(() => expect(result.current.error).toBe('db: 转录读取失败'));

    expect(result.current.loading).toBe(false);
    expect(result.current.sessions).toEqual(SESSIONS_DESC);
  });
});

// ---------------------------------------------------------------------------
// 来源筛选参数化（desktop-change-session-visibility，AC-3）：source 第二参
// 显式必填，'all' 在取数层映射 null 不过滤；切换即重查（effect 依赖增
// source）；显式刷新模式不变（无轮询无订阅回归）。
// ---------------------------------------------------------------------------

describe('useAgentRunHistory：来源筛选参数化（AC-3）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  function sessionsCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
  }

  it("source='change' 挂载 → invoke agent_sessions 携 { root, source: 'change', sourceRef: null }，sessions 承接清单", async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'change'));

    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    expect(sessionsCalls()).toHaveLength(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
    });
  });

  it("source='all' 挂载 → agent_sessions 携 source: null（不过滤），混合来源清单原样承接", async () => {
    const mixed = [
      summary(ID_LATEST, 300),
      {
        ...summary('ses-change-1', 200),
        row: {
          ...summary('ses-change-1', 200).row,
          provenance: { source: 'change', sourceRef: 'add-feature/implement/executor/1' },
        },
      },
    ];
    invokeMock.mockResolvedValue(mixed);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'all'));

    await waitFor(() => expect(result.current.sessions).toEqual(mixed));

    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: null,
      sourceRef: null,
    });
  });

  it("source 变化（rerender 'debug' → 'change'）→ effect 重查：agent_sessions 按新 source 重发", async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    type SourceProp = { source: 'debug' | 'change' | 'all' };
    const { rerender } = renderHook((props: SourceProp) => useAgentRunHistory(ROOT, props.source), {
      initialProps: { source: 'debug' },
    });
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    rerender({ source: 'change' });
    await waitFor(() => expect(sessionsCalls()).toHaveLength(2));

    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
    });
  });

  it('refresh() 重取携当前 source（显式刷新模式不变）；取数稳定后静置无新增调用（无轮询无订阅回归）', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'change'));
    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));
    expect(sessionsCalls()).toHaveLength(1);

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(sessionsCalls()).toHaveLength(2));
    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
    });

    // 静置窗口：挂载取数稳定后无轮询无订阅（零新增 agent_sessions 调用）
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(sessionsCalls()).toHaveLength(2);
  });
});
