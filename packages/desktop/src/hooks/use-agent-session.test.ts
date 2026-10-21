import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../types/dto';
import { useAgentSession } from './use-agent-session';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：invoke 按命令名 + 调用序可编程（session_detail /
// agent_session_transcript / agent_session_watch / agent_start / agent_stop），
// Channel mock 捕获 onmessage 供测试直投信封（event / record）；transport、适配
// 层折叠与 `@ai-sdk/react` useChat 全部真实组合（hook 即链路入口）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const SESSION_ID = 'ses-1-1727000000000';
const TS = 1727000000000;

/** store `SessionConfigSnapshot` 快照线形态（camelCase；缺键即类型红）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function turn(
  turnId: number,
  status: TurnSummary['status'],
  sessionId: string = SESSION_ID,
): TurnSummary {
  return {
    turnId,
    sessionId,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 900,
    numTurns: status === 'running' ? null : 1,
    costUsd: status === 'running' ? null : 0.1,
    durationMs: status === 'running' ? null : 500,
    error: null,
  };
}

function summary(
  turns: TurnSummary[],
  inputTokens: number | null = null,
  id: string = SESSION_ID,
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: 'sdk-1',
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: 3,
        model: 'm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
      provenance: { source: 'debug', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens,
      outputTokens: null,
    },
    turns,
  };
}

function textEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function userEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function turnDoneEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 1,
    durationMs: 400,
    costUsd: 0.1,
    usage: {},
    sessionId: 'sdk-1',
  };
}

function deltaEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

/** toolUse + 跨消息 toolResult（收口收敛「单卡无双卡」的 fixture）。 */
function toolUseEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }],
    parentToolUseId: null,
  };
}

function toolResultEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'tool',
    blocks: [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }],
    parentToolUseId: null,
  };
}

// ---------------------------------------------------------------------------
// 可编程 IPC 应答（按命令名分发）
// ---------------------------------------------------------------------------

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  /** 订阅命令应答：deferred = 挂起（测试直投信封后手动落定）/ resolve / reject */
  watch: 'deferred' | 'resolve' | 'reject';
  start: TurnSummary | Error;
}

let ipc: IpcState;

/** 挂起的 `agent_session_watch` 落定器（deferred 态；断流重订用例的手动开关）。 */
const watchResolvers: (() => void)[] = [];

function resolveWatch(): void {
  const resolve = watchResolvers.shift();
  resolve?.();
}

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: summary([turn(11, 'completed')]),
    transcript: [userEvent(0, '首轮提问'), textEvent(1, '首轮结论'), turnDoneEvent(2)],
    watch: 'deferred',
    start: turn(13, 'running'),
    ...overrides,
  };
  ChannelMock.instances.length = 0;
  watchResolvers.length = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'session_detail') {
      return ipc.detail instanceof Error ? Promise.reject(ipc.detail) : Promise.resolve(ipc.detail);
    }
    if (command === 'agent_session_transcript') {
      return ipc.transcript instanceof Error
        ? Promise.reject(ipc.transcript)
        : Promise.resolve(ipc.transcript);
    }
    if (command === 'agent_session_watch') {
      if (ipc.watch === 'reject') return Promise.reject(new Error('watch 命令失败'));
      if (ipc.watch === 'resolve') return Promise.resolve(null);
      return new Promise<null>((resolve) => {
        watchResolvers.push(() => resolve(null));
      });
    }
    if (command === 'agent_start') {
      return ipc.start instanceof Error ? Promise.reject(ipc.start) : Promise.resolve(ipc.start);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  resetIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('尚未建立流（无 Channel 实例）');
  return instance;
}

function deliverEvent(event: AgentEvent): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'event', event });
  });
}

function deliverRecord(record: TurnSummary): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

function callsOf(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

function argsOf(name: string, index = -1): Record<string, unknown> {
  const calls = callsOf(name);
  const call = index < 0 ? calls.at(-1) : calls[index];
  if (!call) throw new Error(`${name} 未被调用`);
  return call[1] as Record<string, unknown>;
}

/** 消息文本集合（对话透镜的 text 部件，顺序保真）。 */
function messageTexts(messages: readonly { parts: readonly { type: string }[] }[]): string[] {
  return messages.flatMap((message) =>
    message.parts
      .filter((part) => part.type === 'text')
      .map((part) => (part as { text?: string }).text ?? ''),
  );
}

function messageIds(messages: readonly { id: string }[]): string[] {
  return messages.map((message) => message.id);
}

interface Params {
  root: string | null;
  sessionId: string | null;
  source?: string;
  refreshKey?: number;
  /** 来源内定位（未提供即 null——既有消费面零漂移） */
  sourceRef?: string | null;
  /** 会话执行根（未提供即 null → 后端回落 root） */
  cwd?: string | null;
}

async function mounted(overrides: Partial<Params> = {}) {
  const params: Params = {
    root: ROOT,
    sessionId: SESSION_ID,
    source: 'debug',
    ...overrides,
  };
  const rendered = renderHook(
    (props: Params) =>
      useAgentSession({
        root: props.root,
        sessionId: props.sessionId,
        source: props.source ?? 'debug',
        sourceRef: props.sourceRef,
        cwd: props.cwd,
        refreshKey: props.refreshKey,
      }),
    { initialProps: params },
  );
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

function rerenderWith(
  rerender: (props: Params) => void,
  overrides: Partial<Params>,
): Promise<void> {
  return act(async () => {
    rerender({ root: ROOT, sessionId: SESSION_ID, source: 'debug', ...overrides });
  });
}

// ---------------------------------------------------------------------------
// 重放装载与库态派生
// ---------------------------------------------------------------------------

describe('useAgentSession：重放装载与库态派生', () => {
  it('session_detail + agent_session_transcript resolve → session / chain / events 落位，messages 为密封折叠 + 轮行交错', async () => {
    const { result } = await mounted();

    expect(argsOf('session_detail', 0)).toEqual({ root: ROOT, sessionId: SESSION_ID });
    expect(argsOf('agent_session_transcript', 0)).toEqual({
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.session?.row.id).toBe(SESSION_ID);
    expect(result.current.chain).toEqual([turn(11, 'completed')]);
    expect(result.current.events).toHaveLength(3);
    expect(result.current.loading).toBe(false);
    // 密封事件逐条成消息 + TurnDone 位后插入对应轮行（尾部补齐）
    expect(messageIds(result.current.messages)).toEqual(['evt-0', 'evt-1', 'evt-2', 'turn-11']);
    expect(messageTexts(result.current.messages)).toEqual(['首轮提问', '首轮结论']);
  });

  it('running 由轮行 status 派生（库态），非 chat.status 替代；全部收口时为 false', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const running = await mounted();
    expect(running.result.current.running).toBe(true);

    resetIpc({ detail: summary([turn(11, 'completed')]) });
    const done = await mounted();
    expect(done.result.current.running).toBe(false);
  });

  it('session_detail 返回 null → 空会话（不虚构会话、零转录查询）', async () => {
    resetIpc({ detail: null });
    const { result } = await mounted();

    expect(result.current.session).toBeNull();
    expect(result.current.chain).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(callsOf('agent_session_transcript')).toHaveLength(0);
    expect(result.current.loading).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 状态驱动挂流（AC-2）
// ---------------------------------------------------------------------------

describe('useAgentSession：状态驱动挂流（AC-2）', () => {
  it('库态存在运行中轮行 → 恰一次订阅（session_detail → agent_session_watch 各一次），Channel delta 累积可见', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();

    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(argsOf('agent_session_watch')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      sessionId: SESSION_ID,
      sinceSeq: 2,
    });

    deliverEvent(deltaEvent(3, '流式累积正文'));
    await waitFor(() => expect(messageTexts(result.current.messages)).toContain('流式累积正文'));
  });

  it('轮行全收口 → 零订阅零轮询（推进假计时 5min 计数不增）', async () => {
    vi.useFakeTimers();
    const { result } = await mounted();
    expect(result.current.running).toBe(false);
    const before = invokeMock.mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });

    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(invokeMock.mock.calls.length, '零定时刷新 / 零事件订阅').toBe(before);
  });

  it('sinceSeq 游标口径（D8）：首次订阅取重放最大 seq（含 delta 不入库时为零事件则 0）', async () => {
    resetIpc({
      detail: summary([turn(11, 'running')]),
      transcript: [textEvent(0, '旧文'), turnDoneEvent(7)],
    });
    await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(argsOf('agent_session_watch').sinceSeq, '重放最大 seq 推高游标').toBe(7);

    resetIpc({ detail: summary([turn(11, 'running')]), transcript: [] });
    await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(argsOf('agent_session_watch').sinceSeq, '零事件游标为 0').toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 单流在挂与发送守卫（AC-3）
// ---------------------------------------------------------------------------

describe('useAgentSession：单流在挂与发送守卫（AC-3）', () => {
  it('sendMessage 发起（发起流在挂）→ 期间 agent_session_watch 零调用，增量只注入一份，Record 收口后 status 复位', async () => {
    const { result } = await mounted();
    act(() => {
      result.current.sendMessage({ prompt: '第二轮提问', permissionMode: 'bypassPermissions' });
    });
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(callsOf('agent_session_watch'), '发起流在挂不另建订阅').toHaveLength(0);
    expect(argsOf('agent_start')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      prompt: '第二轮提问',
      permissionMode: 'bypassPermissions',
      sessionId: SESSION_ID,
      source: 'debug',
      sourceRef: null,
      agent: null,
      cwd: null,
    });

    // 同轮增量只注入一份（文本无重复、气泡数不翻倍）
    deliverEvent(deltaEvent(3, '唯此一份'));
    await waitFor(() => expect(messageTexts(result.current.messages)).toContain('唯此一份'));
    const occurrences = messageTexts(result.current.messages).filter((text) => text === '唯此一份');
    expect(occurrences).toHaveLength(1);

    deliverRecord(turn(13, 'completed'));
    await waitFor(() => expect(result.current.running).toBe(false));
  });

  it('库态 running 时 sendMessage → 零 agent_start invoke（旁观误发被拦）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    act(() => {
      result.current.sendMessage({ prompt: '不该发出', permissionMode: 'default' });
    });
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(0);
    expect(result.current.running).toBe(true);
  });

  it('会话 id 未确立（镜像与参数皆 null）→ sessionId 传 null（New 语义，不新铸 id）', async () => {
    const { result } = await mounted({ sessionId: null });
    act(() => {
      result.current.sendMessage({ prompt: '开新会话', permissionMode: 'default' });
    });
    await act(async () => {});
    expect(argsOf('agent_start').sessionId).toBeNull();
  });

  it('发送组装：镜像未确立时 sessionId 回落 params.sessionId（Continue 语义不新铸）', async () => {
    resetIpc({ detail: null });
    const { result } = await mounted();
    act(() => {
      result.current.sendMessage({ prompt: '续接既有会话', permissionMode: 'default', agent: 7 });
    });
    await act(async () => {});

    expect(argsOf('agent_start')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      prompt: '续接既有会话',
      permissionMode: 'default',
      sessionId: SESSION_ID,
      source: 'debug',
      sourceRef: null,
      agent: 7,
      cwd: null,
    });
  });
});

// ---------------------------------------------------------------------------
// 收口以重放收敛（AC-4）与失败回落（D16）
// ---------------------------------------------------------------------------

describe('useAgentSession：收口以重放收敛（AC-4）', () => {
  it('订阅流收 Record 关流 → 以重放重建 messages，形状与直接重开该会话一致（含跨消息 tool 单卡）', async () => {
    resetIpc({
      detail: summary([turn(11, 'running')]),
      transcript: [userEvent(0, '首轮提问'), toolUseEvent(1), turnDoneEvent(2)],
    });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 收口：库内转录已补齐（toolResult 落库在另一条消息），轮行转 completed
    ipc.detail = summary([turn(11, 'completed')]);
    ipc.transcript = [
      userEvent(0, '首轮提问'),
      toolUseEvent(1),
      turnDoneEvent(2),
      toolResultEvent(3),
    ];
    deliverRecord(turn(11, 'completed'));

    await waitFor(() =>
      expect(messageIds(result.current.messages)).toEqual([
        'evt-0',
        'evt-1',
        'evt-2',
        'turn-11',
        'evt-3',
      ]),
    );
    // 跨消息 tool 配对收敛为单卡（无双卡残留）
    const toolParts = result.current.messages.flatMap((message) =>
      message.parts.filter((part) => part.type === 'tool-Bash'),
    );
    expect(toolParts).toHaveLength(1);
    expect(toolParts[0]).toMatchObject({
      state: 'output-available',
      output: { content: '目录内容', isError: false },
    });
    expect(callsOf('agent_session_watch')).toHaveLength(1);
  });

  it('Record 后重放查询 reject → 回落镜像折叠（messages 与镜像一致、不抛错），错误面携重放失败串', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 收口后重放查询失败（库读取异常）
    ipc.detail = new Error('db: 重放失败');
    ipc.transcript = new Error('db: 转录失败');
    deliverRecord(turn(11, 'completed'));

    await waitFor(() => expect(result.current.running).toBe(false));
    // 回落不阻塞、不抛错：messages 仍为镜像折叠（流关闭后 status 复位）
    expect(result.current.error, '重放失败经错误面呈现（不抛错）').toContain('重放失败');
    // 镜像折叠：重放前的密封事件 + TurnDone 位后的轮行（镜像与折叠一致）
    expect(messageIds(result.current.messages)).toEqual(['evt-0', 'evt-1', 'evt-2', 'turn-11']);
  });
});

// ---------------------------------------------------------------------------
// 断流重订（AC-6）与无进展闸（D15）
// ---------------------------------------------------------------------------

describe('useAgentSession：断流重订与无进展闸', () => {
  it('订阅流关闭而库态仍 running → 状态重评估后以重放推高的 sinceSeq 重订（文本不重投）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { rerender, result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    deliverEvent(deltaEvent(5, '断流前增量'));
    await waitFor(() => expect(messageTexts(result.current.messages)).toContain('断流前增量'));

    // 订阅命令 resolve（断流收尾）；库态仍 running → 下一次状态重评估即重订
    await act(async () => {
      resolveWatch();
    });
    await rerenderWith(rerender, { refreshKey: 1 });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(2));

    const rewatch = argsOf('agent_session_watch');
    expect(rewatch.sessionId).toBe(SESSION_ID);
    // 游标口径：重放装载把游标推到库内转录最大 seq（delta 零落库故回落至 2）
    expect(rewatch.sinceSeq, '重订游标 = 重放推高的库内最大 seq').toBe(2);

    // 重订后以库内转录为单源：delta 不入库 → 实时累积文本被重放结果替换且零重复
    expect(
      messageTexts(result.current.messages).filter((text) => text === '断流前增量').length,
      '同一段增量至多出现一次（不重投）',
    ).toBeLessThanOrEqual(1);
    expect(messageTexts(result.current.messages)).toEqual(['首轮提问', '首轮结论']);
    expect(result.current.running).toBe(true);
  });

  it('订阅无进展即结束（未收 Record、游标未前进）→ 阻断重订；refreshKey 变更后解阻并重试一次', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]), watch: 'resolve' });
    const { rerender, result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    await act(async () => {});
    expect(callsOf('agent_session_watch'), '无进展闸阻断自旋').toHaveLength(1);

    await rerenderWith(rerender, { refreshKey: 1 });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(2));
    expect(result.current.running).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 装载失败与命令异常
// ---------------------------------------------------------------------------

describe('useAgentSession：错误面与非自旋', () => {
  it('session_detail reject → error 置位、loading 复位、零订阅（不因错误态误挂流）', async () => {
    resetIpc({ detail: new Error('db: 会话查询失败') });
    const { result } = await mounted();

    expect(result.current.error).toContain('会话查询失败');
    expect(result.current.loading).toBe(false);
    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(result.current.session).toBeNull();
  });

  it('agent_session_transcript reject → error 置位、loading 复位、零订阅', async () => {
    resetIpc({
      detail: summary([turn(11, 'running')]),
      transcript: new Error('db: 转录查询失败'),
    });
    const { result } = await mounted();

    expect(result.current.error).toContain('转录查询失败');
    expect(result.current.loading).toBe(false);
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });

  it('agent_session_watch reject → 错误面呈现、不永处 streaming、重订次数有界', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]), watch: 'reject' });
    const { result } = await mounted();

    await waitFor(() => expect(result.current.error).toContain('watch 命令失败'));
    await act(async () => {});
    expect(callsOf('agent_session_watch').length).toBeLessThanOrEqual(2);
    // 状态复位：running 仍取库态（未被流状态绑死）
    expect(result.current.running).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 参数边界 / 停止 / 重置 / 卸载 / refreshKey 重评估
// ---------------------------------------------------------------------------

describe('useAgentSession：参数边界与控制动作', () => {
  it('root = null 或 sessionId = null → 零取数零订阅、messages 空、loading false', async () => {
    const noRoot = await mounted({ root: null });
    expect(noRoot.result.current.messages).toEqual([]);
    expect(noRoot.result.current.loading).toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();

    const noSession = await mounted({ sessionId: null });
    expect(noSession.result.current.messages).toEqual([]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('refreshKey 缺省 undefined 与显式数字同语义（挂载各触发一次重放装载）', async () => {
    const first = await mounted();
    expect(callsOf('session_detail')).toHaveLength(1);
    const baseline = callsOf('session_detail').length;
    first.unmount();

    const second = await mounted({ refreshKey: 0 });
    expect(callsOf('session_detail').length, '显式数字同语义（单次装载）').toBe(baseline + 1);
    expect(second.result.current.session?.row.id).toBe(SESSION_ID);
  });

  it('stop() → agent_stop(root, sessionId) 参数逐字；会话 id 未确立时零 invoke', async () => {
    const { result } = await mounted();
    act(() => {
      result.current.stop();
    });
    expect(argsOf('agent_stop')).toEqual({ root: ROOT, sessionId: SESSION_ID });

    invokeMock.mockClear();
    resetIpc({ detail: null });
    const empty = await mounted({ sessionId: null });
    act(() => {
      empty.result.current.stop();
    });
    expect(callsOf('agent_stop')).toHaveLength(0);
  });

  it('reset() → 清 session / chain / events / messages 与游标；重置后可重新装载新会话', async () => {
    const { result, rerender } = await mounted();
    expect(result.current.messages.length).toBeGreaterThan(0);

    act(() => {
      result.current.reset();
    });
    expect(result.current.session).toBeNull();
    expect(result.current.chain).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.messages).toEqual([]);

    // 切换会话：重新装载（旧镜像零残留）
    const otherId = 'ses-2-1727000009999';
    ipc.detail = summary([turn(21, 'completed', otherId)], null, otherId);
    await rerenderWith(rerender, { sessionId: otherId });
    await waitFor(() => expect(result.current.session?.row.id).toBe(otherId));
    expect(result.current.chain).toEqual([turn(21, 'completed', otherId)]);
  });

  it('refreshKey 变更 = 状态重评估：静默态重放重取；转 running 且无在挂流时挂一次，转收口后不重订', async () => {
    // 静默态（无运行中轮行）：refreshKey 变更即重放重取
    const { rerender } = await mounted();
    expect(callsOf('agent_session_watch')).toHaveLength(0);
    const detailBefore = callsOf('session_detail').length;

    ipc.detail = summary([turn(11, 'completed'), turn(12, 'completed')]);
    await rerenderWith(rerender, { refreshKey: 1 });
    await waitFor(() => expect(callsOf('session_detail').length).toBeGreaterThan(detailBefore));
    expect(callsOf('agent_session_watch'), '收口会话零订阅').toHaveLength(0);

    // 库态转 running：无在挂流 → 挂一次
    ipc.detail = summary([turn(11, 'running')]);
    await rerenderWith(rerender, { refreshKey: 2 });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 库态转收口：订阅不重建（在挂流不中途重放，拆订阅由流关闭承担）
    ipc.detail = summary([turn(11, 'completed')]);
    await rerenderWith(rerender, { refreshKey: 3 });
    await act(async () => {});
    expect(callsOf('agent_session_watch'), '收口不重订').toHaveLength(1);
  });

  it('卸载 → 流拆除、零遗留计时器、零后续 invoke', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { unmount } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    vi.useFakeTimers();
    unmount();
    const after = invokeMock.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });
    expect(invokeMock.mock.calls.length, '卸载后零后续 invoke').toBe(after);
  });

  it('切会话：旧会话镜像 / 轮行 / 消息零残留，新会话按自身转录装载', async () => {
    const { result, rerender } = await mounted();
    expect(result.current.session?.row.id).toBe(SESSION_ID);

    const otherId = 'ses-2-1727000009999';
    const otherTurns = [turn(21, 'completed', otherId)];
    ipc.detail = summary(otherTurns, null, otherId);
    ipc.transcript = [textEvent(0, '新会话正文')];
    await rerenderWith(rerender, { sessionId: otherId });
    await waitFor(() => expect(result.current.session?.row.id).toBe(otherId));
    expect(result.current.chain, '旧轮行零残留').toEqual(otherTurns);
    expect(messageTexts(result.current.messages)).toEqual(['新会话正文']);
    // 切会话期间零额外订阅（两条会话均无运行中轮行）
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// early-resolve 镜像形态与订阅寻址（观测点契约：会话镜像 / 轮行 upsert / 游标
// 与目标会话的来源；覆盖「镜像尚未确立即回写」与「重放复位」两条真态）
// ---------------------------------------------------------------------------

describe('useAgentSession：early-resolve 镜像形态与订阅寻址', () => {
  it('early-resolve running 轮行（镜像空）→ 依轮行铸会话镜像：行字段与统计面逐字、轮数现算', async () => {
    resetIpc({ detail: null, transcript: [], start: turn(13, 'running', 'ses-new-1') });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '开新会话', permissionMode: 'default' });
    });
    await act(async () => {});

    // 发起流仍挂着（未收 Record 信封）→ 镜像即 early-resolve 轮行投影
    expect(result.current.session?.row).toEqual({
      id: 'ses-new-1',
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'debug', sourceRef: null },
      createdAt: TS + 13,
      updatedAt: TS + 13,
    });
    expect(result.current.session?.stats).toEqual({
      turnCount: 1,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
    });
    expect(result.current.chain).toEqual([turn(13, 'running', 'ses-new-1')]);
    expect(result.current.running).toBe(true);
  });

  it('同会话轮行 → 镜像行不重建（库读面 updatedAt 保真，只现算轮数与轮行）', async () => {
    const { result } = await mounted();
    expect(result.current.session?.row.updatedAt).toBe(TS + 9);

    act(() => {
      result.current.sendMessage({ prompt: '续接本轮', permissionMode: 'default' });
    });
    await act(async () => {});

    expect(result.current.session?.row.updatedAt, '行字段沿用库读面不回落轮行起点').toBe(TS + 9);
    expect(result.current.session?.row.createdAt).toBe(TS);
    expect(result.current.session?.stats.turnCount).toBe(2);
    expect(result.current.chain.map((row) => row.turnId)).toEqual([11, 13]);
  });

  it('同 turnId 轮行 → 原位置覆盖而非追加（turn 行 upsert 单点）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    deliverRecord({ ...turn(11, 'running'), error: '终态错误串' });

    expect(result.current.chain, '同 turnId 覆盖不新增行').toHaveLength(1);
    expect(result.current.chain[0].error).toBe('终态错误串');
  });

  it('early-resolve 会话 id 确立后 → 订阅寻址取镜像 id（params.sessionId 为 null 也不新铸）', async () => {
    resetIpc({
      detail: summary([turn(13, 'running', 'ses-new-2')], null, 'ses-new-2'),
      transcript: [],
      start: turn(13, 'running', 'ses-new-2'),
    });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '开新会话', permissionMode: 'default' });
    });
    await act(async () => {});
    deliverRecord(turn(13, 'running', 'ses-new-2'));

    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(argsOf('agent_session_watch')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      sessionId: 'ses-new-2',
      sinceSeq: 0,
    });
  });

  it('root 为 null → sendMessage 零 agent_start invoke（发送守卫单点）', async () => {
    const { result } = await mounted({ root: null });

    act(() => {
      result.current.sendMessage({ prompt: '无 root 不发', permissionMode: 'default' });
    });
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(0);
    expect(result.current.messages).toEqual([]);
  });

  it('sessionId 从值切 null → 重放复位：镜像 / 轮行 / 事件 / 消息清空且零新取数', async () => {
    const { rerender, result } = await mounted();
    expect(result.current.session?.row.id).toBe(SESSION_ID);
    const before = invokeMock.mock.calls.length;

    await act(async () => {
      rerender({ root: ROOT, sessionId: null, source: 'debug' });
    });
    await act(async () => {});

    expect(result.current.session).toBeNull();
    expect(result.current.chain).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.messages).toEqual([]);
    expect(result.current.loading).toBe(false);
    expect(invokeMock.mock.calls.length - before, '复位不发新取数').toBe(0);
  });
});

// ---------------------------------------------------------------------------
// sourceRef / cwd 参数面（AC-3 / AC-4 / D8）：两字段以 `?? null` 透传进发送
// body——未提供即 null（调试页 / explore 形态零漂移），提供即逐字到达；重放 /
// 订阅 / 停止面不携执行根与归属。
// ---------------------------------------------------------------------------

/** 发送一轮并等待其落定（发起流由 Record 收口，便于连发两次）。 */
async function sendOnce(
  result: {
    current: { sendMessage: (input: { prompt: string; permissionMode: 'default' }) => void };
  },
  prompt: string,
): Promise<void> {
  act(() => {
    result.current.sendMessage({ prompt, permissionMode: 'default' });
  });
  await act(async () => {});
}

describe('useAgentSession：sourceRef 与 cwd 参数面', () => {
  it('两字段进发送 body：sourceRef / cwd 逐字到达 agent_start', async () => {
    const { result } = await mounted({
      source: 'change',
      sourceRef: 'chg-1',
      cwd: 'C:\\demo\\wt',
    });

    await sendOnce(result, '带执行根一轮');

    expect(argsOf('agent_start')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      prompt: '带执行根一轮',
      permissionMode: 'default',
      sessionId: SESSION_ID,
      source: 'change',
      sourceRef: 'chg-1',
      agent: null,
      cwd: 'C:\\demo\\wt',
    });
  });

  it('未提供即 null：sourceRef 与 cwd 键恒在、值为 null（9 键稳定）', async () => {
    const { result } = await mounted();

    await sendOnce(result, '既有形态');

    const args = argsOf('agent_start');
    expect(Object.keys(args).sort()).toEqual([
      'agent',
      'cwd',
      'onEvent',
      'permissionMode',
      'prompt',
      'root',
      'sessionId',
      'source',
      'sourceRef',
    ]);
    expect(args['sourceRef']).toBeNull();
    expect(args['cwd']).toBeNull();
  });

  it('显式 null 与缺席同值：两次发送的 agent_start 参数对象全等', async () => {
    const absent = await mounted();
    await sendOnce(absent.result, '缺席两字段');
    const absentArgs = argsOf('agent_start');
    absent.unmount();

    const explicit = await mounted({ sourceRef: null, cwd: null });
    await sendOnce(explicit.result, '缺席两字段');
    const explicitArgs = argsOf('agent_start');

    // onEvent 为各流独立 Channel 实例（不参与同值比较），其余 8 键逐字段全等
    expect(explicitArgs).toEqual({ ...absentArgs, onEvent: expect.any(ChannelMock) });
    expect(explicitArgs['cwd']).toBeNull();
    expect(explicitArgs['sourceRef']).toBeNull();
  });

  it('rerender 换 cwd 即生效：第二次发送携新执行根（无陈旧闭包）', async () => {
    const { result, rerender } = await mounted({ cwd: 'C:\\a' });
    await sendOnce(result, '第一次');
    expect(argsOf('agent_start')['cwd']).toBe('C:\\a');

    deliverRecord(turn(13, 'completed'));
    await waitFor(() => expect(result.current.running).toBe(false));

    await rerenderWith(rerender, { cwd: 'C:\\b' });
    await sendOnce(result, '第二次');

    expect(argsOf('agent_start')['cwd']).toBe('C:\\b');
  });

  it('空态首发携 cwd：sessionId 为 null 且执行根在场（New 语义与执行根并存）', async () => {
    const { result } = await mounted({ sessionId: null, cwd: 'C:\\demo\\wt' });

    await sendOnce(result, '开新会话');

    const args = argsOf('agent_start');
    expect(args['sessionId']).toBeNull();
    expect(args['cwd']).toBe('C:\\demo\\wt');
  });

  it('空串原样透传：cwd 为空串时不 trim、不改写为 null（后端按空白回落 root）', async () => {
    const { result } = await mounted({ cwd: '' });

    await sendOnce(result, '空串执行根');

    expect(argsOf('agent_start')['cwd']).toBe('');
  });

  it('重放面不受 cwd 影响：session_detail / 转录参数仍恰 { root, sessionId }', async () => {
    await mounted({ cwd: 'C:\\demo\\wt' });

    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: SESSION_ID });
    expect(argsOf('agent_session_transcript')).toEqual({ root: ROOT, sessionId: SESSION_ID });
    expect(argsOf('session_detail')).not.toHaveProperty('cwd');
    expect(argsOf('session_detail')).not.toHaveProperty('sourceRef');
  });

  it('停止寻址不含两字段：stop() 的 agent_stop 参数恰 { root, sessionId }', async () => {
    const { result } = await mounted({ sourceRef: 'chg-1', cwd: 'C:\\demo\\wt' });

    act(() => {
      result.current.stop();
    });

    expect(argsOf('agent_stop')).toEqual({ root: ROOT, sessionId: SESSION_ID });
  });

  it('非法 cwd 类型（越过类型签名的脏值）→ 发起前拒绝「非法 cwd」、零 agent_start、running 不置位', async () => {
    const { result } = await mounted({ cwd: 42 as unknown as string });

    act(() => {
      result.current.sendMessage({ prompt: '脏执行根', permissionMode: 'default' });
    });
    await act(async () => {});
    await act(async () => {});

    await waitFor(() => expect(result.current.error).toContain('非法 cwd'));
    expect(callsOf('agent_start')).toHaveLength(0);
    expect(result.current.running, '脏值不置运行态').toBe(false);
  });
});
