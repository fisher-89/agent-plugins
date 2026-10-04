import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { useSessionTranscript } from './use-session-transcript';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（session_detail / agent_sessions /
// agent_session_transcript，可切换 resolve / reject 并记录入参——desktop-
// change-session-visibility 扩展 session_detail 直查臂）；agent-adapter 不
// mock：真实实现参与折叠与去重（内部模块不 mock）。覆盖：直查优先（D8）/
// 反查兜底 / sourceRef 定式 / 空闲与空态 / 实时 seq 去重并入 / running 标志 /
// reject（AC-5 转录联动半边 + AC-8 直查半边）。
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
// 可编程 IPC：单查 / 反查清单与转录均可切 reject（字符串 = reject 文本；
// SessionSummary[] 之外可给 null = blank root 空结果透传形态）
// ---------------------------------------------------------------------------

let sessionsResult: SessionSummary[] | string;
let transcriptResult: AgentEvent[] | string;
let detailResult: SessionSummary | null | string;

function mockIpc() {
  sessionsResult = [summary(SESSION_ID, [turn(11, 'completed')])];
  transcriptResult = REPLAY;
  detailResult = summary(SESSION_ID, [turn(11, 'completed')]);
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'session_detail') {
      return typeof detailResult === 'string'
        ? Promise.reject(detailResult)
        : Promise.resolve(detailResult);
    }
    if (command === 'agent_sessions') {
      void params;
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
  sessionId: string | null;
  liveEvents: AgentEvent[];
}

function propsWith(overrides: Partial<Params>): Params {
  return { root: ROOT, sourceRef: SOURCE_REF, sessionId: null, liveEvents: [], ...overrides };
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

  it('混合轮行（completed + running 存续）：running 置真（任一轮 running 即 running，非全称量词）', async () => {
    sessionsResult = [summary(SESSION_ID, [turn(10, 'completed'), turn(11, 'running')])];
    const { result } = await mounted();
    await waitFor(() => expect(result.current.running).toBe(true));

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.error).toBeNull();
  });

  it('会话切换后 running 随新会话轮行重算：running 会话 → 终态会话，running 复位 false', async () => {
    sessionsResult = [summary(SESSION_ID, [turn(11, 'running')])];
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.running).toBe(true));

    sessionsResult = [
      summary('ses-next', [turn(21, 'completed')], 'add-feature/implement/executor/2'),
    ];
    transcriptResult = [textMessage(0, '下一会话')];
    rerender(propsWith({ sourceRef: 'add-feature/implement/executor/2' }));
    await waitFor(() => expect(result.current.running).toBe(false));

    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']);
    expect(result.current.error).toBeNull();
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

  it('反查空清单：messages 空、running false、error null、不触发转录重放（节点尚无会话的合法空态）', async () => {
    sessionsResult = [];
    const { result } = await mounted();
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
  });

  it('反查空清单后实时事件到站：无库内会话底座，实时事件不越权呈现在 messages（空会话不虚构转录）', async () => {
    sessionsResult = [];
    const { result, rerender } = await mounted();
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    rerender(propsWith({ liveEvents: [textMessage(2, '无主实时片段')] }));
    await act(async () => {});
    expect(result.current.messages).toEqual([]);
    expect(result.current.error).toBeNull();
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

  it('实时事件乱序到站（seq 3 先于 seq 2）：归并后仍按 seq 严格升序装配（非追加序）', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    rerender(
      propsWith({
        liveEvents: [textMessage(3, '后到 seq 3'), textMessage(2, '先到 seq 2')],
      }),
    );
    await waitFor(() => expect(result.current.messages).toHaveLength(4));

    expect(result.current.messages.map((message) => message.id)).toEqual([
      'evt-0',
      'evt-1',
      'evt-2',
      'evt-3',
    ]);
    // 装配序 = seq 升序：seq 2 文本在 seq 3 之前（乱序到站被排序纠正）
    expect(result.current.messages[2]?.parts[0]).toMatchObject({
      type: 'text',
      text: '先到 seq 2',
    });
    expect(result.current.messages[3]?.parts[0]).toMatchObject({
      type: 'text',
      text: '后到 seq 3',
    });
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

  it('竞态守卫：切走 sourceRef 后旧反查后到不回写 messages（disposed 清理生效）', async () => {
    const resolvers: Array<(sessions: SessionSummary[]) => void> = [];
    const transcriptBySession: Record<string, AgentEvent[]> = {
      'ses-old-ref': [textMessage(0, '旧会话回写')],
      'ses-new-ref': [textMessage(0, '新会话正文')],
    };
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'agent_sessions') {
        return new Promise<SessionSummary[]>((resolve) => {
          resolvers.push(resolve);
        });
      }
      if (command === 'agent_session_transcript') {
        const id = (params?.sessionId as string | undefined) ?? '';
        return Promise.resolve(transcriptBySession[id] ?? []);
      }
      return Promise.resolve(null);
    });
    const { result, rerender } = renderHook((input: Params) => useSessionTranscript(input), {
      initialProps: propsWith({}),
    });

    rerender(propsWith({ sourceRef: 'add-feature/implement/executor/2' }));
    expect(resolvers).toHaveLength(2);

    // 新 ref 先落定
    await act(async () => {
      resolvers[1]?.([summary('ses-new-ref', [turn(21, 'completed')])]);
    });
    await waitFor(() =>
      expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']),
    );
    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '新会话正文',
    });

    // 旧 ref 后落定：不回写旧会话转录
    await act(async () => {
      resolvers[0]?.([summary('ses-old-ref', [turn(11, 'completed')])]);
    });
    await act(async () => {});
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']);
    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '新会话正文',
    });
    expect(result.current.error).toBeNull();
  });

  it('竞态守卫：切走后旧查询 reject 不置入过期 error（disposed 门禁覆盖 catch 臂）', async () => {
    const gates: Array<{
      resolve: (sessions: SessionSummary[]) => void;
      reject: (cause: unknown) => void;
    }> = [];
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions'
        ? new Promise<SessionSummary[]>((resolve, reject) => {
            gates.push({ resolve, reject });
          })
        : Promise.resolve(REPLAY),
    );
    const { result, rerender } = renderHook((input: Params) => useSessionTranscript(input), {
      initialProps: propsWith({}),
    });

    rerender(propsWith({ sourceRef: 'add-feature/implement/executor/2' }));
    expect(gates).toHaveLength(2);

    // 新 ref 先落定成功
    await act(async () => {
      gates[1]?.resolve([summary('ses-new', [turn(21, 'completed')])]);
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    // 旧 ref 后 reject：error 保持 null（过期失败不呈现）
    await act(async () => {
      gates[0]?.reject('db: 旧查询失败');
    });
    await act(async () => {});
    expect(result.current.error).toBeNull();
    expect(result.current.messages).toHaveLength(2);
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

    expect(result.current.messages).toEqual([]);
  });

  it('非字符串 reject（Error 实例）：String 归一后 cause 原样透传（不吞错、不替换为通用文案）', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions'
        ? Promise.reject(new Error('db: 连接中断'))
        : Promise.resolve(REPLAY),
    );
    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toBe('Error: db: 连接中断'));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 直查优先（desktop-change-session-visibility，AC-8 / D8）：槽位 sessionId 在
// 场走 session_detail 单查 → running 自轮行推导 → agent_session_transcript 重
// 放；agent_sessions 恰零调用（有 id 不走反查）；直查 Err 错误态不回退反查
//（回退仅限槽位缺席，防同 ref 多会话歧义回流）。
// ---------------------------------------------------------------------------

describe('useSessionTranscript：直查优先（D8）', () => {
  function detailCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'session_detail');
  }

  function sessionsCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
  }

  it('sessionId 在场：invoke session_detail 携 { root, sessionId } → 轮行推导 → 转录重放装配 messages；agent_sessions 恰零调用（直查优先）', async () => {
    const { result } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    expect(detailCalls()).toEqual([['session_detail', { root: ROOT, sessionId: 'ses-slot-1' }]]);
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: 'ses-slot-1',
    });
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0', 'evt-1']);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('直查应答轮行含 running → running 置真（面板头部流式状态与反查路径同一推导式）', async () => {
    detailResult = summary('ses-slot-1', [turn(11, 'running')]);
    const { result } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });

    await waitFor(() => expect(result.current.running).toBe(true));
    expect(result.current.error).toBeNull();
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('直查混合轮行（completed + running）：running 置真；切至全终态新会话后复位 false', async () => {
    detailResult = summary('ses-slot-1', [turn(10, 'completed'), turn(11, 'running')]);
    const { result, rerender } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    detailResult = summary('ses-slot-2', [turn(21, 'completed'), turn(22, 'failed')]);
    transcriptResult = [textMessage(0, '新会话终态')];
    rerender(propsWith({ sessionId: 'ses-slot-2' }));
    await waitFor(() => expect(result.current.running).toBe(false));

    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']);
    expect(result.current.error).toBeNull();
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('直查路径转录重放 reject：error 呈现 cause 原文、messages 空（单查成功不豁免重放失败）', async () => {
    transcriptResult = 'db: 转录读取失败';
    const { result } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(result.current.error).toBe('db: 转录读取失败'));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('sessionId 变化（rerender）→ 直查新会话重放（选中联动，与 sourceRef 变化重查同型）', async () => {
    const { result, rerender } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    detailResult = summary('ses-slot-2', [turn(21, 'completed')]);
    transcriptResult = [textMessage(0, '新会话输出')];
    rerender(propsWith({ sessionId: 'ses-slot-2' }));
    await waitFor(() =>
      expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']),
    );

    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '新会话输出',
    });
    expect(detailCalls()).toHaveLength(2);
    expect(detailCalls()[1]).toEqual(['session_detail', { root: ROOT, sessionId: 'ses-slot-2' }]);
  });

  it('直查模式下 liveEvents seq 归并：以直查重放为底去重并入（assembleTranscript 语义与反查路径一致）', async () => {
    const { result, rerender } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    rerender(
      propsWith({
        sessionId: 'ses-slot-1',
        sourceRef: null,
        liveEvents: [textMessage(1, '重复 seq 1'), textMessage(2, '直查实时片段')],
      }),
    );
    await waitFor(() => expect(result.current.messages).toHaveLength(3));

    expect(result.current.messages.map((message) => message.id)).toEqual([
      'evt-0',
      'evt-1',
      'evt-2',
    ]);
    expect(result.current.messages[2]?.parts[0]).toMatchObject({
      type: 'text',
      text: '直查实时片段',
    });
  });

  it('session_detail reject（悬挂 id）：error 呈现 + messages 空，agent_sessions 恰零调用（直查 Err 不静默回退反查）', async () => {
    detailResult = '会话不存在: id=ses-gone';
    const { result } = await mounted({
      sessionId: 'ses-gone',
      sourceRef: 'add-feature/implement/executor/1',
    });
    await waitFor(() => expect(result.current.error).toBe('会话不存在: id=ses-gone'));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('sessionDetail 应答 null（blank root 空结果透传）：空态呈现，不发起 agent_session_transcript', async () => {
    detailResult = null;
    const { result } = await mounted({
      sessionId: 'ses-slot-1',
      sourceRef: null,
    });
    await waitFor(() => expect(detailCalls()).toHaveLength(1));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
  });

  it('sessionId 与 sourceRef 双 null（decision 槽位缺席）：清空态零 invoke（不误挂他 attempt 会话）', async () => {
    const { result } = await mounted({ sessionId: null, sourceRef: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
  });
});
