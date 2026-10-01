import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { useSessionTranscript } from './use-session-transcript';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_sessions / agent_session_transcript，
// 可切换 resolve / reject 并记录入参）；agent-adapter 不 mock：真实实现参与
// 折叠与去重（内部模块不 mock）。覆盖：反查重放装载 / sourceRef 定式 /
// 空闲与空态 / 实时 seq 去重并入 / running 标志 / reject（AC-5 转录联动半边）。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\flow';
const SOURCE_REF = 'add-feature/implement/executor/1';
const SESSION_ID = 'ses-exec-1';
const TS = 1727000000000;

const REPLAY = [runStarted(0), textMessage(1, '首轮结论')];

function turn(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: SESSION_ID,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 999,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    error: null,
  };
}

function summary(id: string, turns: TurnSummary[], sourceRef: string = SOURCE_REF): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: { engine: 'cli', model: null, permissionMode: 'bypassPermissions' },
      provenance: { source: 'change', sourceRef },
      createdAt: TS,
      updatedAt: TS + 1,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
    },
    turns,
  };
}

function textMessage(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: SESSION_ID,
    tools: [],
    mcpServers: [],
  };
}

// ---------------------------------------------------------------------------
// 可编程 IPC：反查清单与转录均可切 reject（字符串 = reject 文本）
// ---------------------------------------------------------------------------

let sessionsResult: SessionSummary[] | string;
let transcriptResult: AgentEvent[] | string;

function mockIpc() {
  sessionsResult = [summary(SESSION_ID, [turn(11, 'completed')])];
  transcriptResult = REPLAY;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_sessions') {
      return typeof sessionsResult === 'string'
        ? Promise.reject(sessionsResult)
        : Promise.resolve(sessionsResult);
    }
    if (command === 'agent_session_transcript') {
      return typeof transcriptResult === 'string'
        ? Promise.reject(transcriptResult)
        : Promise.resolve(transcriptResult);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

interface Params {
  root: string | null;
  sourceRef: string | null;
  liveEvents: AgentEvent[];
}

function propsWith(overrides: Partial<Params>): Params {
  return { root: ROOT, sourceRef: SOURCE_REF, liveEvents: [], ...overrides };
}

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((input: Params) => useSessionTranscript(input), {
    initialProps: propsWith(overrides),
  });
  await act(async () => {});
  return rendered;
}

function sessionsCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
}

describe('useSessionTranscript：反查重放装载（AC-5 重放半边）', () => {
  it('反查命中：agent_sessions（source=change + sourceRef 精确）→ 转录重放 → agent-adapter 折叠 messages', async () => {
    const { result } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: SOURCE_REF,
    });
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0', 'evt-1']);
    expect(result.current.messages[0]?.parts[0]).toMatchObject({ type: 'data-run-started' });
    expect(result.current.messages[1]).toMatchObject({
      role: 'assistant',
      parts: [{ type: 'text', text: '首轮结论' }],
    });
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('反查参数定式：source 恒为 change、sourceRef 原样透传、root 寻址（exact-match 契约）', async () => {
    await mounted({ sourceRef: 'add-feature/test-gen/evaluator/3' });
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    expect(sessionsCalls()[0]).toEqual([
      'agent_sessions',
      { root: ROOT, source: 'change', sourceRef: 'add-feature/test-gen/evaluator/3' },
    ]);
  });

  it('sourceRef 变化即重查：新 sourceRef 反查 + 对应转录重放（运行步节点选中联动）', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    const decisionRef = 'add-feature/dev-design/decision/1';
    sessionsResult = [summary('ses-decision-1', [turn(21, 'completed')], decisionRef)];
    transcriptResult = [textMessage(0, '决策会话输出')];
    rerender(propsWith({ sourceRef: decisionRef }));
    await waitFor(() =>
      expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']),
    );

    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '决策会话输出',
    });
    const calls = sessionsCalls();
    expect(calls).toHaveLength(2);
    expect(calls[1]).toEqual([
      'agent_sessions',
      { root: ROOT, source: 'change', sourceRef: decisionRef },
    ]);
  });

  it('同 sourceRef 多会话（重发同 attempt 的罕见形态）：以清单末位会话重放', async () => {
    sessionsResult = [
      summary('ses-old', [turn(1, 'completed')]),
      summary(SESSION_ID, [turn(2, 'completed')]),
    ];
    transcriptResult = [textMessage(0, '最近会话')];
    const { result } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(1));

    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.messages[0]?.parts[0]).toMatchObject({ type: 'text', text: '最近会话' });
  });
});

describe('useSessionTranscript：running 标志（面板头部流式状态）', () => {
  it('轮行含 running → true；全部终态 → false（两形态切换）', async () => {
    sessionsResult = [summary(SESSION_ID, [turn(11, 'running')])];
    const running = await mounted();
    await waitFor(() => expect(running.result.current.running).toBe(true));
    running.unmount();

    sessionsResult = [summary(SESSION_ID, [turn(11, 'completed')])];
    const done = await mounted();
    await waitFor(() => expect(done.result.current.running).toBe(false));
  });
});

describe('useSessionTranscript：空闲与空态（边界）', () => {
  it('sourceRef null（未选中节点）：零 invoke、messages 空、running false、error null', async () => {
    const { result } = await mounted({ sourceRef: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('root null：零 invoke、空态（未选定 workspace）', async () => {
    const { result } = await mounted({ root: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('反查空清单：messages 空、error null、不触发转录重放（节点尚无会话的合法空态）', async () => {
    sessionsResult = [];
    const { result } = await mounted();
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    expect(result.current.messages).toEqual([]);
    expect(result.current.error).toBeNull();
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
  });
});

describe('useSessionTranscript：实时去重并入（AC-5 实时半边）', () => {
  it('重放已含 seq 的实时事件不重复并入；新 seq 事件追加并按 seq 升序装配', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    rerender(propsWith({ liveEvents: [textMessage(1, '重复 seq 1'), textMessage(2, '新片段')] }));
    await waitFor(() => expect(result.current.messages).toHaveLength(3));

    expect(result.current.messages.map((message) => message.id)).toEqual([
      'evt-0',
      'evt-1',
      'evt-2',
    ]);
    // seq 1 仍为重放原文（不覆盖）；seq 2 为实时并入
    expect(result.current.messages[1]?.parts[0]).toMatchObject({ type: 'text', text: '首轮结论' });
    expect(result.current.messages[2]?.parts[0]).toMatchObject({ type: 'text', text: '新片段' });
  });

  it('重放未就绪时实时事件静默等待：messages 保持空不崩；重放到达后以重放为底', async () => {
    let resolveSessions: (value: SessionSummary[]) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_sessions') {
        return new Promise<SessionSummary[]>((resolve) => {
          resolveSessions = resolve;
        });
      }
      return Promise.resolve(REPLAY);
    });
    const { result, rerender } = renderHook((input: Params) => useSessionTranscript(input), {
      initialProps: propsWith({}),
    });

    rerender(propsWith({ liveEvents: [textMessage(2, '早到片段')] }));
    await act(async () => {});
    expect(result.current.messages).toEqual([]);

    await act(async () => {
      resolveSessions([summary(SESSION_ID, [turn(11, 'completed')])]);
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0', 'evt-1']);
  });

  it('liveEvents 空数组：不触发重装配（messages 保持重放结果）', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    rerender(propsWith({ liveEvents: [] }));
    await act(async () => {});
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0', 'evt-1']);
  });
});

describe('useSessionTranscript：invoke reject（面板可感知错误）', () => {
  it('反查 reject：error 呈现、messages 保持空', async () => {
    sessionsResult = 'db: 会话查询失败';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toBe('db: 会话查询失败'));

    expect(result.current.messages).toEqual([]);
  });

  it('转录重放 reject：error 呈现、不崩', async () => {
    transcriptResult = 'db: 转录读取失败';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toBe('db: 转录读取失败'));
  });
});
