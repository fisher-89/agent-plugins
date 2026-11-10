import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../lib/ipc-drift';
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
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
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

function textEvent(seq: number, text: string, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function userEvent(seq: number, text: string, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function turnDoneEvent(seq: number, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
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

/** run 启动事件（旧转录的组界锚：`turnId` 全 null 时按它切组） */
function runStartedEvent(seq: number, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 'sdk-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function deltaEvent(seq: number, text: string, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

/** toolUse + 跨消息 toolResult（收口收敛「单卡无双卡」的 fixture）。 */
function toolUseEvent(seq: number, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }],
    parentToolUseId: null,
  };
}

function toolResultEvent(seq: number, turnId: number | null = 11): AgentEvent {
  return {
    seq,
    turnId,
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
    // 折叠：该轮提问 + 恰一条 assistant（末条）+ 逐事件 aux system；
    // 轮行挂 assistant 的 metadata.record（不再产 data-run-record 消息）
    const ids = messageIds(result.current.messages);
    expect(ids.at(-1)).toBe('turn-11');
    expect(ids).toContain('turn-11-user');
    expect(ids.filter((id) => id.startsWith('evt-'))).toHaveLength(1);
    expect(messageTexts(result.current.messages)).toEqual(['首轮提问', '首轮结论']);
    expect(result.current.messages.at(-1)?.metadata?.record).toEqual(turn(11, 'completed'));
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
// 实时流跨消息工具配对（结果事件就地补全，不另起消息）
// ---------------------------------------------------------------------------

describe('useAgentSession：实时流跨消息工具配对', () => {
  /** 消息序列内的 Bash 工具部件（类型收窄免断言，断言以 toMatchObject 取值） */
  function bashToolParts(messages: readonly { parts: readonly { type: string }[] }[]) {
    return messages.flatMap((message) => message.parts.filter((part) => part.type === 'tool-Bash'));
  }

  it('toolUse 与随后 toolResult 合住同一消息：单张工具卡 output-available，结果事件不另起气泡', async () => {
    const { result } = await mounted();
    act(() => {
      result.current.sendMessage({ prompt: '跑个命令', permissionMode: 'bypassPermissions' });
    });
    await act(async () => {});

    // 新跑即新轮：turnId 与原会话已重放的轮不同（同轮号只属同一轮的多次响应）
    deliverEvent(toolUseEvent(3, 12));
    await waitFor(() => {
      const parts = bashToolParts(result.current.messages);
      expect(parts).toHaveLength(1);
      expect(parts[0]).toMatchObject({ state: 'input-available', input: { command: 'ls' } });
    });

    deliverEvent(toolResultEvent(4, 12));
    await waitFor(() => {
      const parts = bashToolParts(result.current.messages);
      expect(parts).toHaveLength(1);
      expect(parts[0]).toMatchObject({
        state: 'output-available',
        input: { command: 'ls' },
        output: { content: '目录内容', isError: false },
      });
    });

    // 结果事件不另起消息：本轮恰一条 assistant（turn-12），结果就地补全该消息
    expect(result.current.messages.filter((message) => message.id === 'turn-12')).toHaveLength(1);
    expect(bashToolParts(result.current.messages)).toHaveLength(1);
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

    await waitFor(() => {
      const ids = messageIds(result.current.messages);
      // 提问 + 恰一条 assistant（末条）+ 逐事件 aux（TurnDone 与 toolResult 各一）
      expect(ids.at(-1)).toBe('turn-11');
      expect(ids).toContain('turn-11-user');
      // toolResult 事件为 tool 角色：并入 assistant 消息，不产独立 system 消息
      expect(ids.filter((id) => id.startsWith('evt-'))).toHaveLength(1);
    });
    // 跨消息 tool 配对收敛为单卡（无双卡残留）：output-available 即重放已落地
    await waitFor(() => {
      const toolParts = result.current.messages.flatMap((message) =>
        message.parts.filter((part) => part.type === 'tool-Bash'),
      );
      expect(toolParts).toHaveLength(1);
      expect(toolParts[0]).toMatchObject({
        state: 'output-available',
        output: { content: '目录内容', isError: false },
      });
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
    await waitFor(() =>
      expect(result.current.error, '重放失败经错误面呈现（不抛错）').toContain('重放失败'),
    );
    // 镜像折叠：重放前的密封事件折叠（提问 + assistant 末条 + aux system）
    await waitFor(() => {
      const ids = messageIds(result.current.messages);
      expect(ids.at(-1)).toBe('turn-11');
      expect(ids).toContain('turn-11-user');
    });
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
    // （messages 通知有节流窗：以终态等待替换落地）
    await waitFor(() =>
      expect(messageTexts(result.current.messages)).toEqual(['首轮提问', '首轮结论']),
    );
    expect(messageTexts(result.current.messages)).not.toContain('断流前增量');
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
    // messages 通知有节流窗：清空以终态等待
    await waitFor(() => expect(result.current.messages).toEqual([]));

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
    // messages 通知有节流窗：新会话转录以终态等待
    await waitFor(() => expect(messageTexts(result.current.messages)).toEqual(['新会话正文']));
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

  it.each([
    ['root 为 null', { root: null }],
    ['会话已在挂流（isStreaming 守卫）', {}],
  ])(
    '%s → sendMessage 零 agent_start 且零本地提问推入（不产生孤儿提问）',
    async (label, overrides) => {
      resetIpc({ detail: summary([turn(11, 'running')]) });
      const { result } = await mounted(overrides as Partial<Params>);
      if (label.includes('isStreaming')) {
        // 库态 running 即已有在挂订阅流：守卫先于推入
        await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
      }
      const before = messageIds(result.current.messages);

      act(() => {
        result.current.sendMessage({ prompt: '守卫应拦下', permissionMode: 'default' });
      });
      await act(async () => {});

      expect(callsOf('agent_start')).toHaveLength(0);
      // 零本地推入：消息 id 集合不变、无 pending 临时 id
      expect(messageIds(result.current.messages)).toEqual(before);
      expect(messageIds(result.current.messages).some((id) => id.startsWith('pending-user-'))).toBe(
        false,
      );
    },
  );

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
    // messages 通知有节流窗：复位清空以终态等待
    await waitFor(() => expect(result.current.messages).toEqual([]));
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

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`sessionDetail` /
// `agentSessionTranscript` / `agentStop` 的参数面放行与结果面 soft-report。
// ---------------------------------------------------------------------------

describe('useAgentSession：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('恢复会话 → sessionDetail 与 agentSessionTranscript 各 1 次、参数面放行、结果干净零漂移', async () => {
    const { result } = await mounted();
    await waitFor(() => expect(result.current.messages.length).toBeGreaterThan(0));

    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: SESSION_ID });
    expect(argsOf('agent_session_transcript')).toEqual({
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(driftCount()).toBe(0);
  });

  it('agentStop(root, sessionId) → payload 键名逐字不变、结果 null 零漂移', async () => {
    const { result } = await mounted();
    await waitFor(() => expect(result.current.messages.length).toBeGreaterThan(0));

    act(() => {
      result.current.stop();
    });
    await waitFor(() =>
      expect(invokeMock.mock.calls.some(([name]) => name === 'agent_stop')).toBe(true),
    );

    expect(argsOf('agent_stop')).toEqual({ root: ROOT, sessionId: SESSION_ID });
    expect(driftCount()).toBe(0);
  });

  it('sessionDetail reject → 既有 error 文案（String(err)）逐字不变、零漂移', async () => {
    resetIpc({ detail: new Error('会话不存在：ses-1') });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toBe('Error: 会话不存在：ses-1');
    expect(driftCount()).toBe(0);
  });

  it('转录结果面脏（事件 seq 给字符串）→ 漂移恰 1 条 + 既有重放逻辑按原值继续（零 reject）', async () => {
    resetIpc({
      transcript: [{ ...textEvent(1, '脏事件'), seq: '1' } as unknown as AgentEvent],
    });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('agentSessionTranscript');
    expect(driftReports()[0].path).toBe('#/0/seq');
    expect(result.current.error).toBeNull();
  });

  it.each([
    ['root 为 null', { root: null }],
    ['sessionId 为 null', { sessionId: null }],
  ])('%s → 零 session 类取数、零漂移（既有前置检查与取消语义不变）', async (_label, overrides) => {
    await mounted(overrides);
    await act(async () => {});

    expect(invokeMock.mock.calls.filter(([name]) => name === 'session_detail')).toHaveLength(0);
    expect(driftCount()).toBe(0);
  });

  it('转录空数组 → 结果面放行、零漂移、既有空态不变', async () => {
    resetIpc({ transcript: [] });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.events).toEqual([]);
    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 提问气泡两起点（AC-2 / D5）：发起流本地推入副本 → 提问事件到达时归一；
// 订阅流（无本地副本）在提问事件到达时以 `turn-<turnId>-user` 追加并置于该轮
// assistant 之前。两路不变式：同一提问恰一条气泡、消息数不因归一增加。
// ---------------------------------------------------------------------------

/** 提问气泡的 id 集合（`turn-<turnId>-user` / `evt-<seq>` 兜底 / 临时 `pending-user-*`） */
function userIds(messages: readonly { id: string; role: string }[]): string[] {
  return messages.filter((message) => message.role === 'user').map((message) => message.id);
}

describe('useAgentSession：提问气泡两起点（AC-2）', () => {
  it('发起流本地推入：sendMessage 后先有一条 user 消息（无 chunk 亦然），此刻无 assistant', async () => {
    resetIpc({ detail: null, transcript: [] });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '同步可见的提问', permissionMode: 'default' });
    });
    await act(async () => {});

    // 本地副本结构上先于任何 chunk：此刻未投递任何信封，提问已可见
    await waitFor(() => expect(userIds(result.current.messages)).toHaveLength(1));
    const question = result.current.messages.find((message) => message.role === 'user');
    expect(question?.parts).toEqual([{ type: 'text', text: '同步可见的提问' }]);
    expect(
      result.current.messages.filter((message) => message.role === 'assistant'),
      '尚无任何 chunk',
    ).toHaveLength(0);
    // body 八字段逐字不变（既有参数面）
    expect(argsOf('agent_start')).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      prompt: '同步可见的提问',
      permissionMode: 'default',
      sessionId: null,
      source: 'debug',
      sourceRef: null,
      agent: null,
      cwd: null,
    });
  });

  it('提问归一：该轮提问事件到达 → id 变 turn-13-user、metadata 取事件 seq、消息总数不增', async () => {
    resetIpc({ detail: null, transcript: [] });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '待归一提问', permissionMode: 'default' });
    });
    await act(async () => {});
    await waitFor(() => expect(userIds(result.current.messages)).toHaveLength(1));

    deliverEvent(userEvent(5, '待归一提问', 13));
    await act(async () => {});

    await waitFor(() => expect(userIds(result.current.messages)).toEqual(['turn-13-user']));
    // 提问数不增（id 原地替换，不追加第二条）
    expect(userIds(result.current.messages)).toHaveLength(1);
    expect(
      messageTexts(result.current.messages).filter((text) => text === '待归一提问'),
    ).toHaveLength(1);
    const question = result.current.messages.find((message) => message.id === 'turn-13-user');
    expect(question?.metadata).toEqual({ seq: 5, parentToolUseId: null });
    // 临时 pending id 不残留
    expect(messageIds(result.current.messages).some((id) => id.startsWith('pending-user-'))).toBe(
      false,
    );
  });

  it('提问归一后仍在该轮 assistant 之前（顺序不因归一改变）', async () => {
    resetIpc({ detail: null, transcript: [] });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '顺序判据', permissionMode: 'default' });
    });
    await act(async () => {});
    deliverEvent(userEvent(0, '顺序判据', 13));
    deliverEvent(textEvent(1, '答一句', 13));
    await act(async () => {});

    await waitFor(() => expect(messageIds(result.current.messages)).toContain('turn-13'));
    const ids = messageIds(result.current.messages);
    expect(ids).toContain('turn-13-user');
    expect(ids.indexOf('turn-13-user')).toBeLessThan(ids.indexOf('turn-13'));
  });

  it('订阅流旁观（无本地副本）：提问事件 → 以 turn-<turnId>-user 追加在该轮 assistant 之前', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    await waitFor(() => expect(messageIds(result.current.messages)).toContain('turn-11'));

    deliverEvent(textEvent(3, '新轮正文', 12));
    await act(async () => {});
    deliverEvent(userEvent(4, '旁观看到的提问', 12));
    await act(async () => {});

    await waitFor(() => expect(userIds(result.current.messages)).toContain('turn-12-user'));
    const ids = messageIds(result.current.messages);
    expect(ids.indexOf('turn-12-user')).toBeLessThan(ids.indexOf('turn-12'));
  });

  it('订阅流多轮：两轮各自提问事件 → 两条提问气泡各就位（不串轮、不重复）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    deliverEvent(userEvent(3, '轮十二提问', 12));
    deliverEvent(textEvent(4, '轮十二正文', 12));
    await act(async () => {});
    deliverEvent(userEvent(5, '轮十三提问', 13));
    deliverEvent(textEvent(6, '轮十三正文', 13));
    await act(async () => {});

    await waitFor(() =>
      expect(userIds(result.current.messages)).toEqual([
        'turn-11-user',
        'turn-12-user',
        'turn-13-user',
      ]),
    );
    // 提问不串轮、不重复（同一提问恰一条）；assistant 侧不因提问事件新增第二条
    const assistantCount = result.current.messages.filter(
      (message) => message.role === 'assistant',
    ).length;
    expect(assistantCount).toBeGreaterThanOrEqual(2);
  });

  it('同一轮提问事件重投（重订 / 收口竞态）→ 归一后仍恰一条气泡（id 替换幂等）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    deliverEvent(userEvent(3, '重复投递的提问', 12));
    deliverEvent(userEvent(3, '重复投递的提问', 12));
    await act(async () => {});

    await waitFor(() =>
      expect(userIds(result.current.messages).filter((id) => id === 'turn-12-user')).toHaveLength(
        1,
      ),
    );
  });

  it('连续两轮发送：pending 计数各自独立，第二轮归一为 turn-<第二轮 turnId>-user 且不串号', async () => {
    resetIpc({ detail: null, transcript: [] });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '第一轮', permissionMode: 'default' });
    });
    await act(async () => {});
    deliverEvent(userEvent(0, '第一轮', 13));
    await act(async () => {});
    // 收口后重放读同一批库内转录（轮行转 completed）
    ipc.detail = summary([turn(13, 'completed')]);
    ipc.transcript = [userEvent(0, '第一轮', 13)];
    deliverRecord(turn(13, 'completed'));
    await waitFor(() => expect(result.current.running).toBe(false));
    await waitFor(() => expect(userIds(result.current.messages)).toEqual(['turn-13-user']));

    act(() => {
      result.current.sendMessage({ prompt: '第二轮', permissionMode: 'default' });
    });
    await act(async () => {});
    deliverEvent(userEvent(1, '第二轮', 14));
    await act(async () => {});

    await waitFor(() =>
      expect(userIds(result.current.messages)).toEqual(['turn-13-user', 'turn-14-user']),
    );
    expect(messageTexts(result.current.messages).filter((text) => text.startsWith('第'))).toEqual([
      '第一轮',
      '第二轮',
    ]);
  });

  it.each([
    { label: '空串', prompt: '' },
    { label: '超长（>1000 chars）', prompt: '问'.repeat(1200) },
    { label: '特殊字符（换行 / NUL / emoji）', prompt: '第一行\n第二行\u0000结束😀' },
    // 越过 `AgentSessionSendInput.prompt: string` 签名的脏值：守卫层不在 hook 侧，
    // 故不抛错且本地副本按原值就位（不夹取、不升级为空串）
    { label: '越签名脏值（undefined）', prompt: undefined as unknown as string },
  ])('prompt 边界 $label：本地 text 部件逐字保留不 trim、不抛错', async ({ prompt }) => {
    resetIpc({ detail: null, transcript: [] });
    const { result } = await mounted({ sessionId: null });

    expect(() => {
      act(() => {
        result.current.sendMessage({ prompt, permissionMode: 'default' });
      });
    }).not.toThrow();
    await act(async () => {});

    await waitFor(() =>
      expect(result.current.messages.filter((message) => message.role === 'user')).toHaveLength(1),
    );
    const question = result.current.messages.find((message) => message.role === 'user');
    expect(question?.parts).toHaveLength(1);
    const part = question?.parts[0] as { type: string; text?: string } | undefined;
    expect(part?.type).toBe('text');
    // 逐字保留：空串不升级、长串不截断、特殊字符不转义、脏值不改写
    expect(part?.text).toBe(prompt);
  });
});

// ---------------------------------------------------------------------------
// 断流兜底（AC-5 兜底面）：发起流未收 Record 且提问事件也不可达时，`onFinish`
// 以镜像折叠整表替换——本地推入的临时 `pending-user-*` 副本由 fold 覆盖掉，
// 不残留任何临时 id（也不虚构孤儿提问）。
// ---------------------------------------------------------------------------

describe('useAgentSession：断流兜底（onFinish fold）', () => {
  it('发起流断流 → onFinish 折叠兜底，临时 pending id 不残留、无孤儿提问', async () => {
    resetIpc({
      detail: null,
      transcript: [],
      start: new Error('启动失败：引擎未就绪'),
    });
    const { result } = await mounted({ sessionId: null });

    act(() => {
      result.current.sendMessage({ prompt: '断流提问', permissionMode: 'default' });
    });
    await act(async () => {});

    // 前置事实：本地副本已随发送推入（守卫放行，agent_start 携该 prompt 发出）
    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start').prompt).toBe('断流提问');

    // agent_start reject → envelope.fail → 流注错 → makeRequest 的 finally 必调 onFinish
    await waitFor(() => expect(result.current.error).toContain('启动失败'));
    await waitFor(() => expect(result.current.messages).toEqual([]));

    const ids = messageIds(result.current.messages);
    expect(
      ids.some((id) => id.startsWith('pending-user-')),
      '临时 pending id 不残留',
    ).toBe(false);
    // 本地副本语义由 fold 覆盖：镜像零事件零轮行 → 折叠结果为空表（不虚构提问）
    expect(
      result.current.messages.filter((message) => message.role === 'user'),
      '无孤儿提问',
    ).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 收口一致（AC-5）：实时流后的 id 序列与收口后（onFinish fold → 重放覆盖）逐元素相同
// ---------------------------------------------------------------------------

describe('useAgentSession：收口前后 id 序列一致（AC-5）', () => {
  it('实时流（delta + 密封 + Record）后的提问与 assistant id 在收口后逐元素保持', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 实时流：提问 + 增量 + 密封块
    deliverEvent(userEvent(3, '实时提问', 12));
    deliverEvent(deltaEvent(4, '实时增量', 12));
    deliverEvent(textEvent(5, '实时正文', 12));
    await act(async () => {});
    await waitFor(() => expect(messageIds(result.current.messages)).toContain('turn-12'));
    const liveIds = messageIds(result.current.messages);

    // 收口：库内转录补齐为同一批密封事件，轮行转 completed
    ipc.detail = summary([turn(11, 'completed'), turn(12, 'completed')]);
    ipc.transcript = [
      userEvent(0, '首轮提问'),
      textEvent(1, '首轮结论'),
      turnDoneEvent(2),
      userEvent(3, '实时提问', 12),
      textEvent(5, '实时正文', 12),
    ];
    deliverRecord(turn(12, 'completed'));

    await waitFor(() => expect(result.current.running).toBe(false));
    await waitFor(() => expect(messageIds(result.current.messages)).toContain('turn-12'));

    // 收口后不整段重挂：同一提问与同一 assistant 各恰一条
    expect(liveIds.filter((id) => id === 'turn-12-user')).toHaveLength(1);
    expect(liveIds.filter((id) => id === 'turn-12')).toHaveLength(1);
    const settledIds = messageIds(result.current.messages);
    expect(settledIds.filter((id) => id === 'turn-12-user')).toHaveLength(1);
    expect(settledIds.filter((id) => id === 'turn-12')).toHaveLength(1);
    // 收口后 assistant 的轮行来自库（completed）
    expect(
      result.current.messages.find((message) => message.id === 'turn-12')?.metadata?.record?.status,
    ).toBe('completed');
  });
});

// ---------------------------------------------------------------------------
// 旧转录与零内容轮（AC-8 / AC-9 前提）
// ---------------------------------------------------------------------------

describe('useAgentSession：旧转录与零内容轮', () => {
  it('三轮重放（正常 / 停止 / 正常）：id 序列为 turn-<id>-user / turn-<id> 形状、停止轮 record 就位、轮行零错位（AC-7）', async () => {
    resetIpc({
      detail: summary([turn(11, 'completed'), turn(12, 'stopped'), turn(13, 'completed')]),
      transcript: [
        runStartedEvent(0, 11),
        userEvent(1, '第一问', 11),
        textEvent(2, '第一答', 11),
        turnDoneEvent(3, 11),
        // 停止轮：停止路径不发 TurnDone
        runStartedEvent(4, 12),
        userEvent(5, '第二问', 12),
        runStartedEvent(6, 13),
        userEvent(7, '第三问', 13),
        textEvent(8, '第三答', 13),
        turnDoneEvent(9, 13),
      ],
    });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    const assistants = result.current.messages.filter((message) => message.role === 'assistant');
    expect(assistants.map((message) => message.id)).toEqual(['turn-11', 'turn-12', 'turn-13']);
    expect(assistants.map((message) => message.metadata?.record?.status)).toEqual([
      'completed',
      'stopped',
      'completed',
    ]);
    // 停止轮 parts 为空仍就位（空内容轮恒一条）
    expect(assistants[1]?.parts).toEqual([]);
    expect(userIds(result.current.messages)).toEqual([
      'turn-11-user',
      'turn-12-user',
      'turn-13-user',
    ]);
  });

  it('旧转录重放（turnId 全 null）→ id 走 evt-<seq> 兜底、每轮提问 + 一条 assistant、error 为 null', async () => {
    resetIpc({
      detail: summary([turn(1, 'completed'), turn(2, 'completed')]),
      transcript: [
        runStartedEvent(0, null),
        userEvent(1, '旧问一', null),
        textEvent(2, '旧答一', null),
        runStartedEvent(3, null),
        userEvent(4, '旧问二', null),
        textEvent(5, '旧答二', null),
      ],
    });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    const ids = messageIds(result.current.messages);
    expect(userIds(result.current.messages)).toEqual(['evt-1', 'evt-4']);
    // assistant 兜 `evt-<该消息首事件 seq>`：与提问 id 不同源，故不撞
    expect(ids).toEqual(['evt-0', 'evt-1', 'evt-2', 'evt-3', 'evt-4', 'evt-5']);
    expect(messageTexts(result.current.messages)).toEqual(['旧问一', '旧答一', '旧问二', '旧答二']);
    expect(result.current.error).toBeNull();
  });

  it('零内容轮：轮行 stopped 而事件侧无 assistant 密封 → 一条 parts 空的 assistant 且 record 就位', async () => {
    resetIpc({
      detail: summary([turn(11, 'stopped'), turn(12, 'completed')]),
      transcript: [
        userEvent(0, '被停止的提问', 11),
        userEvent(1, '后续提问', 12),
        textEvent(2, '后续答', 12),
      ],
    });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    const stopped = result.current.messages.find((message) => message.id === 'turn-11');
    expect(stopped).toBeDefined();
    expect(stopped?.parts).toEqual([]);
    expect(stopped?.metadata?.record?.status).toBe('stopped');
    // 轮行不错位：第 2 轮轮行落在第 2 轮
    expect(messageIds(result.current.messages).at(-1)).toBe('turn-12');
    expect(
      result.current.messages.find((message) => message.id === 'turn-12')?.metadata?.record?.status,
    ).toBe('completed');
  });

  it('空会话（events = [] 且 turns = []）→ messages 为 []；运行中轮行不就地虚构消息', async () => {
    resetIpc({ detail: summary([]), transcript: [] });
    const empty = await mounted();
    await waitFor(() => expect(empty.result.current.loading).toBe(false));
    expect(empty.result.current.messages).toEqual([]);
    empty.unmount();

    // 库态含 running 轮行（挂流可建）：轮行只进 chain / running 派生，不就地折叠出消息
    resetIpc({ detail: summary([turn(20, 'running')]), transcript: [] });
    const { result } = await mounted();
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    expect(result.current.chain.map((row) => row.turnId)).toEqual([20]);
    expect(result.current.running).toBe(true);
    // 零事件轮由折叠恒产一条 parts 空 assistant（AC-9 的「空内容轮恒产一条消息」）；
    // 不因 running 轮行额外虚构第二条
    const assistants = result.current.messages.filter((message) => message.role === 'assistant');
    expect(assistants).toHaveLength(1);
    expect(assistants[0]?.id).toBe('turn-20');
    expect(assistants[0]?.parts).toEqual([]);
    expect(assistants[0]?.metadata?.record?.status).toBe('running');
  });
});
