import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionPage, SessionSummary } from '../../../types/dto';
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

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function summary(id: string, updatedAt: number): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: 7,
        model: 'glm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
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

/** 后端分页信封：`{ sessions, total }`（total = 过滤集全量计数）。 */
function pageOf(sessions: SessionSummary[], total = sessions.length) {
  return { sessions, total };
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('refresh() 重取仍携同一 root：invoke("agent_sessions") 重发且 sessions 按后端降序原样承接', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
        offset: 0,
        limit: 20,
      });
      expect(result.current.sessions).toEqual(SESSIONS_DESC);
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBeNull();
    });
  });

  it('openSession(id) → invoke("agent_session_transcript", { root, sessionId }) → events 更新、selectedSessionId 跟随', async () => {
    invokeMock.mockResolvedValue(pageOf([]));
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
        : Promise.resolve(pageOf(SESSIONS_DESC)),
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
    invokeMock.mockResolvedValue(pageOf([]));
    let resolveTranscript: (events: AgentEvent[]) => void = () => {};
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_session_transcript'
        ? new Promise<AgentEvent[]>((resolve) => {
            resolveTranscript = resolve;
          })
        : Promise.resolve(pageOf(SESSIONS_DESC)),
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
    let resolveSessions: (page: SessionPage) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise<SessionPage>((resolve) => {
          resolveSessions = resolve;
        }),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    await act(async () => {
      resolveSessions(pageOf(SESSIONS_DESC));
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('清单轨道竞态守卫：切走 source 后旧查询落定不回写 sessions、不置入旧 error', async () => {
    const resolvers: Array<(page: SessionPage) => void> = [];
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions'
        ? new Promise<SessionPage>((resolve) => {
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
      resolvers[1]?.(pageOf(fresh));
    });
    await waitFor(() => expect(result.current.sessions).toEqual(fresh));

    // 旧 source 后落定（reject）：不得回写 sessions、不得置入旧错误
    await act(async () => {
      resolvers[0]?.(undefined as unknown as SessionPage);
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
    invokeMock.mockResolvedValue(pageOf([]));
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
      offset: 0,
      limit: 20,
    });
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: ID_LATEST,
    });
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(1);
    expect(result.current.selectedSessionId).toBe(ID_LATEST);
  });

  it('agent_sessions 返回 [] → sessions 为空数组不崩（绑定切换不改变空态）', async () => {
    invokeMock.mockResolvedValue(pageOf([]));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => {
      expect(result.current.sessions).toEqual([]);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('refresh / openSession reject → error 置串、loading 复位、sessions 清单不受影响（错误路径不变）', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'change'));

    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    expect(sessionsCalls()).toHaveLength(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
      offset: 0,
      limit: 20,
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
    invokeMock.mockResolvedValue(pageOf(mixed));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'all'));

    await waitFor(() => expect(result.current.sessions).toEqual(mixed));

    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: null,
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
  });

  it("source 变化（rerender 'debug' → 'change'）→ effect 重查：agent_sessions 按新 source 重发", async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
      offset: 0,
      limit: 20,
    });
  });

  it('refresh() 重取携当前 source（显式刷新模式不变）；取数稳定后静置无新增调用（无轮询无订阅回归）', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
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
      offset: 0,
      limit: 20,
    });

    // 静置窗口：挂载取数稳定后无轮询无订阅（零新增 agent_sessions 调用）
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(sessionsCalls()).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// 分页（AC：agent_sessions 携 offset / limit，SessionPage 信封 total 推页界）
// ---------------------------------------------------------------------------

describe('useAgentRunHistory：分页取数与页界导出', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('挂载即携 offset: 0 / limit: 20（PAGE_SIZE 常量口径），total 推 totalPages', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC, 45));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
    expect(result.current.total).toBe(45);
    expect(result.current.page).toBe(1);
    expect(result.current.totalPages).toBe(3);
  });

  it('setPage(3) → invoke 携 offset: 40（(page-1) * PAGE_SIZE），sessions 承接该页', async () => {
    const page3 = [summary('ses-p3', 42)];
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions' ? Promise.resolve(pageOf(page3, 45)) : Promise.resolve([]),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.totalPages).toBe(3));

    act(() => {
      result.current.setPage(3);
    });
    await waitFor(() => expect(result.current.page).toBe(3));
    await waitFor(() => expect(result.current.sessions).toEqual(page3));
    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
      offset: 40,
      limit: 20,
    });
  });

  it('边界：setPage 越界夹在 [1, totalPages]，nextPage / prevPage 到界不动', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC, 45));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.totalPages).toBe(3));

    act(() => {
      result.current.setPage(100);
    });
    await waitFor(() => expect(result.current.page).toBe(3));

    act(() => {
      result.current.nextPage();
    });
    await waitFor(() => expect(result.current.page).toBe(3));

    act(() => {
      result.current.prevPage();
    });
    await waitFor(() => expect(result.current.page).toBe(2));

    act(() => {
      result.current.prevPage();
    });
    await waitFor(() => expect(result.current.page).toBe(1));

    act(() => {
      result.current.prevPage();
    });
    await waitFor(() => expect(result.current.page).toBe(1));
  });

  it('total 收缩致当前页越界：夹回末页重取（本页空结果不落），totalPages 收敛', async () => {
    let total = 45;
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions'
        ? Promise.resolve(pageOf([summary('ses-x', 9)], total))
        : Promise.resolve([]),
    );
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.totalPages).toBe(3));

    act(() => {
      result.current.setPage(3);
    });
    await waitFor(() => expect(result.current.page).toBe(3));

    // 总量收缩到 1 页 → 第 3 页越界 → 回末页（第 1 页）重取
    total = 5;
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.page).toBe(1));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.total).toBe(5);
    expect(result.current.totalPages).toBe(1);
  });

  it('source 变化回第 1 页：高页位切 source 后 offset 归 0', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC, 45));
    type SourceProp = { source: 'debug' | 'change' };
    const { result, rerender } = renderHook(
      (props: SourceProp) => useAgentRunHistory(ROOT, props.source),
      { initialProps: { source: 'debug' } },
    );
    await waitFor(() => expect(result.current.totalPages).toBe(3));
    act(() => {
      result.current.setPage(3);
    });
    await waitFor(() => expect(result.current.page).toBe(3));

    rerender({ source: 'change' });
    await waitFor(() => expect(result.current.page).toBe(1));
    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
  });

  it('refresh() 停留当前页：高页位刷新 offset 不变（按当前筛选与当前页重取）', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC, 45));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.totalPages).toBe(3));
    act(() => {
      result.current.setPage(2);
    });
    await waitFor(() => expect(result.current.page).toBe(2));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.page).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
      offset: 20,
      limit: 20,
    });
  });

  it('边界：空清单 total=0 → totalPages=1（页界不塌 0），sessions 空数组', async () => {
    invokeMock.mockResolvedValue(pageOf([]));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.total).toBe(0);
    expect(result.current.totalPages).toBe(1);
    expect(result.current.page).toBe(1);
    expect(result.current.sessions).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

/** id 边界形态的会话行 fixture（agentInstanceId 携 0 / -1 / MAX_SAFE_INTEGER）。 */
function summaryWithInstanceId(
  id: string,
  agentInstanceId: number,
  updatedAt: number,
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId,
        model: 'glm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
      provenance: { source: 'debug', sourceRef: null },
      createdAt: 1727000000000,
      updatedAt,
    },
    stats: { turnCount: 0, totalDurationMs: 0, inputTokens: null, outputTokens: null },
    turns: [],
  };
}

describe('useAgentRunHistory：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('默认 fixture（新会话语义：具体实例 id）装载后清单镜像快照同形保绿', async () => {
    invokeMock.mockResolvedValue(pageOf(SESSIONS_DESC));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.sessions).toEqual(SESSIONS_DESC));

    const snapshot = result.current.sessions[0].row.configSnapshot as ConfigSnapshotFixture;
    expect(snapshot).toHaveProperty('agentInstanceId', 7);
    expect(snapshot.engine).toBe('sdk');
    expect(result.current.error).toBeNull();
  });

  it('engine cli 历史快照与 agentInstanceId null（存量缺列）形态照常装载不炸（缺键旧形态经类型面拦截）', async () => {
    const legacy: SessionSummary = {
      row: {
        id: 'ses-legacy-cli',
        remoteSessionId: 'cli-1-1727000000002',
        configSnapshot: {
          engine: 'cli',
          agentInstanceId: null,
          model: null,
          permissionMode: 'bypassPermissions',
        } satisfies ConfigSnapshotFixture,
        provenance: { source: 'debug', sourceRef: null },
        createdAt: 1727000000000,
        updatedAt: 200,
      },
      stats: { turnCount: 1, totalDurationMs: 100, inputTokens: null, outputTokens: null },
      turns: [],
    };
    invokeMock.mockResolvedValue(pageOf([legacy]));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));

    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    // 呈现断言容忍两形态：null（存量会话缺列语义）原样透出（不回填不加工）
    expect(result.current.sessions[0].row.configSnapshot).toEqual({
      engine: 'cli',
      agentInstanceId: null,
      model: null,
      permissionMode: 'bypassPermissions',
    });
    expect(result.current.error).toBeNull();
  });

  it('agentInstanceId id 边界（0 / -1 / MAX_SAFE_INTEGER）与清单空 / 单元素 / 多元素载荷同口径承接', async () => {
    // 多元素：id 边界三形态混入（呈现断言容忍，不加工不拒绝）
    const mixed = [
      summaryWithInstanceId('ses-zero', 0, 300),
      summaryWithInstanceId('ses-negative', -1, 200),
      summaryWithInstanceId('ses-max', Number.MAX_SAFE_INTEGER, 100),
    ];
    invokeMock.mockResolvedValue(pageOf(mixed));
    const { result } = renderHook(() => useAgentRunHistory(ROOT, 'debug'));
    await waitFor(() => expect(result.current.sessions).toHaveLength(3));
    expect(
      result.current.sessions.map(
        (session) => (session.row.configSnapshot as ConfigSnapshotFixture).agentInstanceId,
      ),
    ).toEqual([0, -1, Number.MAX_SAFE_INTEGER]);

    // 单元素载荷同口径（list 映射边界）
    invokeMock.mockResolvedValue(pageOf(mixed.slice(0, 1)));
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));
    expect(
      (result.current.sessions[0].row.configSnapshot as ConfigSnapshotFixture).agentInstanceId,
    ).toBe(0);

    // 空清单载荷同口径
    invokeMock.mockResolvedValue(pageOf([]));
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.sessions).toEqual([]));
  });
});
