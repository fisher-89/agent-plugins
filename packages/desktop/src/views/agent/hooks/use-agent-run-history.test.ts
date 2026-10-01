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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));
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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toBe('db: 清单打开失败'));

    expect(result.current.loading).toBe(false);
  });

  it('openSession reject：error 置串、loading 复位、sessions 清单不受影响', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT));
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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    await act(async () => {
      resolveSessions(SESSIONS_DESC);
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('root 为 null（未选定 workspace）：不发起任何 invoke，sessions 保持空态（跳过取数分支）', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(null));

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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));
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
    const { result } = renderHook(() => useAgentRunHistory(ROOT));

    await waitFor(() => {
      expect(result.current.sessions).toEqual([]);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('refresh / openSession reject → error 置串、loading 复位、sessions 清单不受影响（错误路径不变）', async () => {
    invokeMock.mockResolvedValue(SESSIONS_DESC);
    const { result } = renderHook(() => useAgentRunHistory(ROOT));
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
