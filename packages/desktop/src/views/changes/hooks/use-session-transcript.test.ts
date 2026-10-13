import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { useSessionTranscript } from './use-session-transcript';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（session_detail / agent_session_transcript，
// 可切换 resolve / reject 并记录入参——desktop-change-session-visibility 的
// sessionId 单点直查寻址）；agent-adapter 不 mock：真实实现参与折叠与去重
//（内部模块不 mock）。覆盖：直查单点寻址（D8）/ 参数定式 / sessionId null 空闲
// 与空态 / 实时 seq 去重并入 / running 标志 / reject（AC-5 转录联动半边 + AC-8
// 直查半边）；refreshKey 重查键（change run 面实时统一转录库——通知触发重查面）。
// 注：实时 seq 去重并入为归档链专用面（`liveEvents` 参数保留供 archive-panel
// 复用，见 hook 头注释），change run 面恒缺省 → 重放单源。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\flow';
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

function summary(id: string, turns: TurnSummary[]): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: { engine: 'cli', model: null, permissionMode: 'bypassPermissions' },
      provenance: { source: 'change', sourceRef: null },
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
// 可编程 IPC：session_detail 单查与转录重放均可切 reject（字符串 = reject
// 文本；SessionSummary 之外可给 null = blank root 空结果透传形态）
// ---------------------------------------------------------------------------

let transcriptResult: AgentEvent[] | string;
let detailResult: SessionSummary | null | string;

function mockIpc() {
  transcriptResult = REPLAY;
  detailResult = summary(SESSION_ID, [turn(11, 'completed')]);
  invokeMock.mockImplementation((command: string) => {
    if (command === 'session_detail') {
      return typeof detailResult === 'string'
        ? Promise.reject(detailResult)
        : Promise.resolve(detailResult);
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
  sessionId: string | null;
  liveEvents: AgentEvent[];
  /** 变更通知触发的重查键（unify-run-state-persistence：自增即重查转录库） */
  refreshKey?: number;
}

function propsWith(overrides: Partial<Params>): Params {
  return {
    root: ROOT,
    sessionId: SESSION_ID,
    liveEvents: [],
    ...overrides,
  };
}

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((input: Params) => useSessionTranscript(input), {
    initialProps: propsWith(overrides),
  });
  await act(async () => {});
  return rendered;
}

function detailCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'session_detail');
}

describe('useSessionTranscript：sessionId 直查重放装载（AC-5 重放半边）', () => {
  it('直查命中：session_detail（root + sessionId 精确）→ 转录重放 → agent-adapter 折叠 messages', async () => {
    const { result } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: SESSION_ID,
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

  it('直查参数定式：sessionId 原样透传、root 寻址（exact-id 契约，不反查）', async () => {
    await mounted({ sessionId: 'ses-test-gen-3' });
    await waitFor(() => expect(detailCalls()).toHaveLength(1));

    expect(detailCalls()[0]).toEqual([
      'session_detail',
      { root: ROOT, sessionId: 'ses-test-gen-3' },
    ]);
  });

  it('sessionId 变化即重查：新 sessionId 直查 + 对应转录重放（运行步节点选中联动）', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    detailResult = summary('ses-decision-1', [turn(21, 'completed')]);
    transcriptResult = [textMessage(0, '决策会话输出')];
    rerender(propsWith({ sessionId: 'ses-decision-1' }));
    await waitFor(() =>
      expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']),
    );

    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '决策会话输出',
    });
    const calls = detailCalls();
    expect(calls).toHaveLength(2);
    expect(calls[1]).toEqual(['session_detail', { root: ROOT, sessionId: 'ses-decision-1' }]);
  });

  it('sessionId 直查唯一寻址：只重放该 id 的转录（不串其他会话，单点精确无歧义）', async () => {
    transcriptResult = [textMessage(0, '直查会话')];
    const { result } = await mounted({ sessionId: SESSION_ID });
    await waitFor(() => expect(result.current.messages).toHaveLength(1));

    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(invokeMock).not.toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: 'ses-old',
    });
    expect(detailCalls()).toEqual([['session_detail', { root: ROOT, sessionId: SESSION_ID }]]);
    expect(result.current.messages[0]?.parts[0]).toMatchObject({ type: 'text', text: '直查会话' });
  });
});

describe('useSessionTranscript：running 标志（面板头部流式状态）', () => {
  it('轮行含 running → true；全部终态 → false（两形态切换）', async () => {
    detailResult = summary(SESSION_ID, [turn(11, 'running')]);
    const running = await mounted();
    await waitFor(() => expect(running.result.current.running).toBe(true));
    running.unmount();

    detailResult = summary(SESSION_ID, [turn(11, 'completed')]);
    const done = await mounted();
    await waitFor(() => expect(done.result.current.running).toBe(false));
  });

  it('混合轮行（completed + running 存续）：running 置真（任一轮 running 即 running，非全称量词）', async () => {
    detailResult = summary(SESSION_ID, [turn(10, 'completed'), turn(11, 'running')]);
    const { result } = await mounted();
    await waitFor(() => expect(result.current.running).toBe(true));

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.error).toBeNull();
  });

  it('会话切换后 running 随新会话轮行重算：running 会话 → 终态会话，running 复位 false', async () => {
    detailResult = summary(SESSION_ID, [turn(11, 'running')]);
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.running).toBe(true));

    detailResult = summary('ses-next', [turn(21, 'completed')]);
    transcriptResult = [textMessage(0, '下一会话')];
    rerender(propsWith({ sessionId: 'ses-next' }));
    await waitFor(() => expect(result.current.running).toBe(false));

    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']);
    expect(result.current.error).toBeNull();
  });
});

describe('useSessionTranscript：空闲与空态（边界）', () => {
  it('sessionId null（未选中节点）：零 invoke、messages 空、running false、error null、summary null', async () => {
    const { result } = await mounted({ sessionId: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.summary).toBeNull();
  });

  it('root null：零 invoke、空态（未选定 workspace）、summary null', async () => {
    const { result } = await mounted({ root: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.error).toBeNull();
    expect(result.current.summary).toBeNull();
  });

  it('sessionId null：messages 空、running false、error null、summary null、不触发单查与转录重放（无槽位合法空态）', async () => {
    const { result } = await mounted({ sessionId: null });
    await act(async () => {});

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.summary).toBeNull();
    expect(invokeMock.mock.calls.filter(([name]) => name === 'session_detail')).toHaveLength(0);
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
  });

  it('sessionId null 后实时事件到站：无库内会话底座，实时事件不越权呈现在 messages（空会话不虚构转录）', async () => {
    const { result, rerender } = await mounted({ sessionId: null });
    expect(result.current.messages).toEqual([]);

    rerender(propsWith({ sessionId: null, liveEvents: [textMessage(2, '无主实时片段')] }));
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
    let resolveDetail: (value: SessionSummary | null) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'session_detail') {
        return new Promise<SessionSummary | null>((resolve) => {
          resolveDetail = resolve;
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
      resolveDetail(summary(SESSION_ID, [turn(11, 'completed')]));
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

  it('竞态守卫：切走 sessionId 后旧直查后到不回写 messages（disposed 清理生效）', async () => {
    const resolvers: Array<(detail: SessionSummary | null) => void> = [];
    const transcriptBySession: Record<string, AgentEvent[]> = {
      'ses-old-ref': [textMessage(0, '旧会话回写')],
      'ses-new-ref': [textMessage(0, '新会话正文')],
    };
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'session_detail') {
        return new Promise<SessionSummary | null>((resolve) => {
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
      initialProps: propsWith({ sessionId: 'ses-old-ref' }),
    });

    rerender(propsWith({ sessionId: 'ses-new-ref' }));
    expect(resolvers).toHaveLength(2);

    // 新 id 先落定
    await act(async () => {
      resolvers[1]?.(summary('ses-new-ref', [turn(21, 'completed')]));
    });
    await waitFor(() =>
      expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']),
    );
    expect(result.current.messages[0]?.parts[0]).toMatchObject({
      type: 'text',
      text: '新会话正文',
    });

    // 旧 id 后落定：不回写旧会话转录
    await act(async () => {
      resolvers[0]?.(summary('ses-old-ref', [turn(11, 'completed')]));
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
      resolve: (detail: SessionSummary | null) => void;
      reject: (cause: unknown) => void;
    }> = [];
    invokeMock.mockImplementation((command: string) =>
      command === 'session_detail'
        ? new Promise<SessionSummary | null>((resolve, reject) => {
            gates.push({ resolve, reject });
          })
        : Promise.resolve(REPLAY),
    );
    const { result, rerender } = renderHook((input: Params) => useSessionTranscript(input), {
      initialProps: propsWith({ sessionId: 'ses-old' }),
    });

    rerender(propsWith({ sessionId: 'ses-new' }));
    expect(gates).toHaveLength(2);

    // 新 id 先落定成功
    await act(async () => {
      gates[1]?.resolve(summary('ses-new', [turn(21, 'completed')]));
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    // 旧 id 后 reject：error 保持 null（过期失败不呈现）
    await act(async () => {
      gates[0]?.reject('db: 旧查询失败');
    });
    await act(async () => {});
    expect(result.current.error).toBeNull();
    expect(result.current.messages).toHaveLength(2);
  });
});

describe('useSessionTranscript：invoke reject（面板可感知错误）', () => {
  it('session_detail reject：error 呈现、messages 保持空', async () => {
    detailResult = 'db: 会话查询失败';
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
      command === 'session_detail'
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
// 直查单点寻址（desktop-change-session-visibility，AC-8 / D8）：槽位 sessionId
// 在场走 session_detail 单查 → running 自轮行推导 → agent_session_transcript
// 重放；反查路径已整体退役（agent_sessions 恰零调用为红线守卫）。直查 Err 错误
// 态不静默降级（无回退路径，防同 ref 多会话歧义回流）。
// ---------------------------------------------------------------------------

describe('useSessionTranscript：直查单点寻址（D8）', () => {
  function sessionsCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
  }

  it('sessionId 在场：invoke session_detail 携 { root, sessionId } → 轮行推导 → 转录重放装配 messages；agent_sessions 恰零调用（反查退役）', async () => {
    const { result } = await mounted({ sessionId: 'ses-slot-1' });
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

  it('直查应答轮行含 running → running 置真（面板头部流式状态与单查路径同一推导式）', async () => {
    detailResult = summary('ses-slot-1', [turn(11, 'running')]);
    const { result } = await mounted({ sessionId: 'ses-slot-1' });

    await waitFor(() => expect(result.current.running).toBe(true));
    expect(result.current.error).toBeNull();
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('直查混合轮行（completed + running）：running 置真；切至全终态新会话后复位 false', async () => {
    detailResult = summary('ses-slot-1', [turn(10, 'completed'), turn(11, 'running')]);
    const { result, rerender } = await mounted({ sessionId: 'ses-slot-1' });
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
    const { result } = await mounted({ sessionId: 'ses-slot-1' });
    await waitFor(() => expect(result.current.error).toBe('db: 转录读取失败'));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('sessionId 变化（rerender）→ 直查新会话重放（选中联动，单点寻址同型）', async () => {
    const { result, rerender } = await mounted({ sessionId: 'ses-slot-1' });
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

  it('直查模式下 liveEvents seq 归并：以直查重放为底去重并入（assembleTranscript 语义与单查路径一致）', async () => {
    const { result, rerender } = await mounted({ sessionId: 'ses-slot-1' });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    rerender(
      propsWith({
        sessionId: 'ses-slot-1',
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

  it('session_detail reject（悬挂 id）：error 呈现 + messages 空，agent_sessions 恰零调用（直查 Err 不静默降级）', async () => {
    detailResult = '会话不存在: id=ses-gone';
    const { result } = await mounted({ sessionId: 'ses-gone' });
    await waitFor(() => expect(result.current.error).toBe('会话不存在: id=ses-gone'));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(sessionsCalls()).toHaveLength(0);
  });

  it('sessionDetail 应答 null（blank root 空结果透传）：空态呈现、summary null，不发起 agent_session_transcript', async () => {
    detailResult = null;
    const { result } = await mounted({ sessionId: 'ses-slot-1' });
    await waitFor(() => expect(detailCalls()).toHaveLength(1));

    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.summary).toBeNull();
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
  });

  it('sessionId null（槽位缺席）：清空态零 invoke、summary null（不误挂他 attempt 会话）', async () => {
    const { result } = await mounted({ sessionId: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.summary).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// summary 三件套（desktop-drawer-session-column，AC-3 / D4）：返回面增
// `summary: SessionSummary | null`（row + stats + turns）——与 messages 同一次
// 取数（直查 sessionDetail 应答），不重复发 invoke；错误态与空态下 summary 恒
// null（不虚构元信息）。
// ---------------------------------------------------------------------------

describe('useSessionTranscript：summary 三件套（AC-3 / D4）', () => {
  it('直查命中：summary = session_detail 应答三件套逐字段（row / stats / turns），且与 messages 同一次直查取数', async () => {
    const direct = summary('ses-slot-1', [turn(11, 'running')]);
    detailResult = direct;
    transcriptResult = REPLAY;
    const { result } = await mounted({ sessionId: 'ses-slot-1' });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    // 逐字段同源：row 身份面 / provenance / stats / turns 全量透出
    expect(result.current.summary).not.toBeNull();
    expect(result.current.summary?.row.id).toBe('ses-slot-1');
    expect(result.current.summary?.row.remoteSessionId).toBe(direct.row.remoteSessionId);
    expect(result.current.summary?.row.provenance).toEqual(direct.row.provenance);
    expect(result.current.summary?.row.createdAt).toBe(direct.row.createdAt);
    expect(result.current.summary?.stats).toEqual(direct.stats);
    expect(result.current.summary?.turns).toEqual(direct.turns);
    // 与 messages 同一次直查取数：session_detail 恰一次，不重复发 invoke
    expect(detailCalls()).toHaveLength(1);
    expect(result.current.running).toBe(true);
  });

  it('直查命中：summary 三件套随应答透出（row 身份面 + stats + turns 与应答逐字段同源；调试会话 provenance.sourceRef 为 null）', async () => {
    const direct = summary('ses-latest', [turn(12, 'completed')]);
    detailResult = direct;
    transcriptResult = REPLAY;
    const { result } = await mounted({ sessionId: 'ses-latest' });
    await waitFor(() => expect(result.current.messages).toHaveLength(2));

    expect(result.current.summary?.row.id).toBe('ses-latest');
    expect(result.current.summary?.row.provenance).toEqual(direct.row.provenance);
    expect(result.current.summary?.row.provenance.sourceRef).toBeNull();
    expect(result.current.summary?.stats).toEqual(direct.stats);
    expect(result.current.summary?.turns).toEqual(direct.turns);
  });

  it('sessionId 变化重查：summary 随新会话更新（与 messages / running 同一 effect 重查链）', async () => {
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.summary?.row.id).toBe(SESSION_ID));

    const nextId = 'ses-evaluator-1';
    detailResult = summary(nextId, [turn(21, 'completed')]);
    transcriptResult = [textMessage(0, '评估会话输出')];
    rerender(propsWith({ sessionId: nextId }));
    await waitFor(() => expect(result.current.summary?.row.id).toBe(nextId));

    expect(result.current.summary?.row.provenance.sourceRef).toBeNull();
    expect(result.current.messages.map((message) => message.id)).toEqual(['evt-0']);
  });

  it('reject 错误态：error 呈现、summary 保持 null（转录重放失败与单查失败两臂皆不虚构元信息）', async () => {
    transcriptResult = 'db: 转录读取失败';
    const replayFail = await mounted();
    await waitFor(() => expect(replayFail.result.current.error).toBe('db: 转录读取失败'));
    expect(replayFail.result.current.summary).toBeNull();
    replayFail.unmount();

    detailResult = '会话不存在: id=ses-gone';
    const direct = await mounted({ sessionId: 'ses-gone' });
    await waitFor(() => expect(direct.result.current.error).toBe('会话不存在: id=ses-gone'));
    expect(direct.result.current.summary).toBeNull();
  });
});

describe('useSessionTranscript：refreshKey 重查键（unify-run-state-persistence 转录统一）', () => {
  function transcriptCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript');
  }

  it('refreshKey bump → 重新发起同一寻址查询（sessionId 直查优先，通知触发重查面 AC-10）', async () => {
    const { result, rerender } = await mounted({ sessionId: SESSION_ID });
    expect(result.current.messages.length).toBe(REPLAY.length);
    const base = transcriptCalls().length;
    expect(base).toBeGreaterThan(0);

    await act(async () => {
      rerender(propsWith({ sessionId: SESSION_ID, refreshKey: 1 }));
    });
    await act(async () => {});
    expect(transcriptCalls().length, 'refreshKey 自增即重查转录库').toBe(base + 1);
    // 同一寻址参数（sessionId 直查不变）
    const latest = transcriptCalls().at(-1)?.[1] as Record<string, unknown>;
    expect(latest.sessionId).toBe(SESSION_ID);

    await act(async () => {
      rerender(propsWith({ sessionId: SESSION_ID, refreshKey: 2 }));
    });
    await act(async () => {});
    expect(transcriptCalls().length, '再次 bump 再次重查').toBe(base + 2);
  });

  it('refreshKey 不变时寻址键变化仍即时重查（节点切换联动既有语义不因刷新键引入而弱化）', async () => {
    const { rerender } = await mounted({ sessionId: 'ses-a' });
    const base = detailCalls().length;

    await act(async () => {
      rerender(propsWith({ sessionId: 'ses-b', refreshKey: 0 }));
    });
    await act(async () => {});
    expect(detailCalls().length, '寻址键变化即时重查（refreshKey 恒定）').toBe(base + 1);
    const latest = detailCalls().at(-1)?.[1] as Record<string, unknown>;
    expect(latest.sessionId).toBe('ses-b');
  });
});
