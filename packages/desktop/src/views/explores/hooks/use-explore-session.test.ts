import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { buildExplorePrompt } from '../../../lib/explore-stance';
import type { AgentEvent, ExploreRecord, SessionSummary, TurnSummary } from '../../../types/dto';
import { useExploreSession } from './use-explore-session';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_sessions / agent_session_transcript /
// agent_start 记录入参供断言、可切换 reject / pending；agent_stop 记录寻址）；
// Channel mock 为可编程 class（捕获 onmessage，测试直接投递 AgentRunMessage
// 信封——ipc: 'event' / 'record' 双变体——驱动实时流与终态回流）。
// stance 前导不 mock：buildExplorePrompt 以真实实现参与 send 拼接语义断言。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
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
const RECORD_ID = 7;
const SESSION_ID = 'ses-12-1727000000000';

function exploreRecord(): ExploreRecord {
  return {
    id: RECORD_ID,
    root: ROOT,
    name: 'api-retry',
    createdAt: 1727000000000,
    updatedAt: 1727000000000,
  };
}

function turn(
  turnId: number,
  status: TurnSummary['status'] = 'completed',
  sessionId: string = SESSION_ID,
): TurnSummary {
  return {
    turnId,
    sessionId,
    status,
    startedAt: 1727000000000 + turnId,
    finishedAt: status === 'running' ? null : 1727000001000 + turnId,
    numTurns: status === 'running' ? null : 1,
    costUsd: status === 'running' ? null : 0.1,
    durationMs: status === 'running' ? null : 500,
    error: null,
  };
}

function sessionSummary(turns: TurnSummary[]): SessionSummary {
  return {
    row: {
      id: SESSION_ID,
      remoteSessionId: 's-tail',
      configSnapshot: { engine: 'cli', model: null, permissionMode: 'bypassPermissions' },
      provenance: { source: 'explore', sourceRef: String(RECORD_ID) },
      createdAt: 1727000000000,
      updatedAt: 1727000009999,
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

function textEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function turnDoneEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 1,
    durationMs: 500,
    costUsd: 0.1,
    usage: {},
    sessionId: 's-tail',
  };
}

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('send 未创建 Channel');
  return instance;
}

/** 投递实时事件信封（ipc: 'event'）。 */
function deliverEvent(event: AgentEvent) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'event', event });
  });
}

/** 投递终态轮行信封（ipc: 'record'，后端闭流收尾）。 */
function deliverRecord(record: TurnSummary) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

/** 会话 fixture：两轮往复（轮统计行 2 行，全史转录 3 事件）。 */
const sessionFixture = sessionSummary([turn(11), turn(12)]);

let sessionsResult: SessionSummary[] | string;
let transcript: AgentEvent[] | string;
let startResult: TurnSummary | string | null;

function mockIpc() {
  ChannelMock.instances.length = 0;
  sessionsResult = [sessionSummary([turn(11), turn(12)])];
  transcript = [textEvent(0, '首轮结论'), turnDoneEvent(1), textEvent(2, '续轮结论')];
  // 提前 resolve 契约：agent_start 返回 running 轮行（当前会话随 onRecord 确立）
  startResult = turn(13, 'running', 'ses-13-1727000001000');
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'agent_sessions') {
      expect(params).toMatchObject({
        root: ROOT,
        source: 'explore',
        sourceRef: String(RECORD_ID),
      });
      return typeof sessionsResult === 'string'
        ? Promise.reject(sessionsResult)
        : Promise.resolve(sessionsResult);
    }
    if (command === 'agent_session_transcript') {
      if (typeof transcript === 'string') {
        return Promise.reject(transcript);
      }
      return Promise.resolve(transcript);
    }
    if (command === 'agent_start') {
      return typeof startResult === 'string'
        ? Promise.reject(startResult)
        : Promise.resolve(startResult);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

async function mounted(record: ExploreRecord | null = exploreRecord()) {
  const rendered = renderHook(
    (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
    { initialProps: { record } },
  );
  await act(async () => {});
  return rendered;
}

function startCallArgs(): Record<string, unknown> {
  const calls = invokeMock.mock.calls.filter(([name]) => name === 'agent_start');
  const call = calls.at(-1);
  if (!call) throw new Error('agent_start 未被调用');
  return call[1] as Record<string, unknown>;
}

const SEND_INPUT = {
  prompt: '继续追这条线索',
  permissionMode: 'bypassPermissions' as const,
};

describe('useExploreSession：会话还原与全史转录重放（AC-9/AC-5）', () => {
  it('record 有既有会话：挂载发起 agent_sessions 恰一次（携 root 寻址 + 来源二元组）+ agent_session_transcript 全史重放，events 承接转录', async () => {
    const { result } = await mounted();

    expect(result.current.loading).toBe(false);
    const sessionCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
    expect(sessionCalls).toHaveLength(1);
    expect(sessionCalls[0]).toEqual([
      'agent_sessions',
      { root: ROOT, source: 'explore', sourceRef: String(RECORD_ID) },
    ]);
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.chain).toEqual(sessionFixture.turns);
    expect(result.current.events).toEqual(transcript as AgentEvent[]);
    expect(result.current.error).toBeNull();
  });

  it('record 为 null：不还原会话（events 空态、loading 复位），send 为 no-op', async () => {
    const { result } = await mounted(null);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.chain).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.loading).toBe(false);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.running).toBe(false);
  });

  it('agent_session_transcript reject：错误可见不崩（error 置位）', async () => {
    transcript = '转录查询失败';

    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toContain('转录查询失败'));

    expect(result.current.loading).toBe(false);
  });
});

describe('useExploreSession：send 拼接 stance 与当前会话续话（AC-9，D4/D7）', () => {
  it('有会话时 send：prompt 以 stance 前导开头、sessionId 为当前会话、三元组齐全', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    const prompt = String(args.prompt);
    expect(prompt.startsWith(buildExplorePrompt('').slice(0, 40))).toBe(true);
    expect(prompt.endsWith(SEND_INPUT.prompt)).toBe(true);
    expect(args.sessionId).toBe(SESSION_ID);
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBe(String(RECORD_ID));
    expect(args.root).toBe(ROOT);
    expect(args.permissionMode).toBe('bypassPermissions');
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('无会话时 send（首条起会话）：sessionId 不传（null 即 New）、来源三元组仍携带', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.sessionId).toBeNull();
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBe(String(RECORD_ID));
  });

  it('轮终态后：轮序列增长、终态轮行回流复位，二次 send 的 sessionId 更新为新轮行会话', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    // 提前 resolve：running 轮行入链（轮 id 立即可用，会话镜像回写）
    expect(result.current.chain).toHaveLength(3);
    expect(result.current.chain[2]).toEqual(turn(13, 'running', 'ses-13-1727000001000'));

    // 终态轮行经 Channel 信封回流（后端闭流收尾）：链尾推进为终态行
    deliverRecord(turn(13, 'completed', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));
    expect(result.current.chain[2]).toEqual(turn(13, 'completed', 'ses-13-1727000001000'));

    act(() => {
      result.current.send({ ...SEND_INPUT, prompt: '再来一轮' });
    });
    await act(async () => {});

    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(2);
    expect(startCallArgs().sessionId).toBe('ses-13-1727000001000');
  });

  it('尚无会话时 stop()：无寻址目标不发起 invoke（幂等忽略半边）', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_stop')).toHaveLength(0);
  });

  it('运行中重复 send：被抑制（running 门闩），不并发发起第二条轮', async () => {
    let resolveStart: ((value: TurnSummary) => void) | null = null;
    startResult = null;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') {
        return new Promise<TurnSummary>((resolve) => {
          resolveStart = resolve;
        });
      }
      if (command === 'agent_sessions') {
        return Promise.resolve([sessionSummary([turn(11), turn(12)])]);
      }
      if (command === 'agent_session_transcript') {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);

    // 启动 resolve（early resolve running）+ 终态轮行信封回流 → running 复位
    act(() => {
      resolveStart?.(turn(13, 'running', 'ses-13-1727000001000'));
    });
    await act(async () => {});
    deliverRecord(turn(13, 'stopped', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);
  });

  it('send 成功后实时 Channel 事件信封（ipc: event）累积进 events', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    deliverEvent(textEvent(0, '实时片段'));
    await act(async () => {});

    expect(result.current.events).toContainEqual(textEvent(0, '实时片段'));
  });
});

// ---------------------------------------------------------------------------
// stop 透传（停止时序：前端 → invoke("agent_stop") → 后端闭流收尾，
// 前端流 MUST NOT 截断——终态轮行部件经 Channel 回流后 running 复位）。
// ---------------------------------------------------------------------------

describe('useExploreSession → 基建接线：stop 透传', () => {
  it('运行中 stop() → invoke("agent_stop", { root, sessionId }) 会话寻址触达后端；Channel 仍可投递事件与终态轮行', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledWith('agent_stop', {
      root: ROOT,
      sessionId: 'ses-13-1727000001000',
    });

    // 前端流不截断：stop 后事件信封照常入镜像
    deliverEvent(textEvent(0, '停止前已产出'));
    await act(async () => {});
    expect(result.current.events).toContainEqual(textEvent(0, '停止前已产出'));

    // 终态轮行回流：running 经 Record 复位
    deliverRecord(turn(13, 'stopped', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));
    expect(result.current.chain.at(-1)?.status).toBe('stopped');
  });
});

describe('useExploreSession：失败语义（AC-9）', () => {
  it('agent_start reject：error 置位、running 复位、既有事件不丢', async () => {
    const { result } = await mounted();
    const eventsBefore = [...result.current.events];
    expect(eventsBefore.length).toBeGreaterThan(0);

    startResult = '启动失败';
    act(() => {
      result.current.send(SEND_INPUT);
    });

    await waitFor(() => expect(result.current.running).toBe(false));
    expect(result.current.error).toContain('启动失败');
    expect(result.current.events).toEqual(eventsBefore);
    expect(result.current.chain).toEqual(sessionFixture.turns);
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：加载在途态、root 守卫、记录切换竞态（取消抑制）。
// ---------------------------------------------------------------------------

describe('useExploreSession：加载在途态与守卫（AC-9）', () => {
  it('会话查询在途：chain / events 保持初始空态、loading 置位', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_sessions') return new Promise<SessionSummary[]>(() => {});
      return Promise.resolve([]);
    });

    const { result } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});

    expect(result.current.loading).toBe(true);
    expect(result.current.chain).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('root 为 null：send 为 no-op（不发起 agent_start、running 不置位）', async () => {
    const { result } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(null, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.running).toBe(false);
  });
});

describe('useExploreSession：记录切换竞态（取消抑制，AC-9）', () => {
  const recordB: ExploreRecord = { ...exploreRecord(), id: 8, name: 'other-topic' };
  const sessionB = sessionSummary([turn(21, 'completed', 'ses-21-1727000000000')]);

  it('旧记录迟到的会话 resolve 不覆盖新会话', async () => {
    let resolveA: (value: SessionSummary[]) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'agent_sessions') {
        if (params?.sourceRef === '7') {
          return new Promise<SessionSummary[]>((resolve) => {
            resolveA = resolve;
          });
        }
        return Promise.resolve([sessionB]);
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});

    rerender({ record: recordB });
    await act(async () => {});
    expect(result.current.chain).toEqual(sessionB.turns);

    await act(async () => {
      resolveA([sessionFixture]);
    });
    expect(result.current.chain).toEqual(sessionB.turns);
  });

  it('旧记录迟到的会话 reject 不置错误、不污染新会话', async () => {
    let rejectA: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'agent_sessions') {
        if (params?.sourceRef === '7') {
          return new Promise<SessionSummary[]>((_resolve, reject) => {
            rejectA = reject;
          });
        }
        return Promise.resolve([sessionB]);
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});

    rerender({ record: recordB });
    await act(async () => {});

    await act(async () => {
      rejectA(new Error('迟到的会话查询失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.chain).toEqual(sessionB.turns);
  });
});

// ---------------------------------------------------------------------------
// 补遗：返回形状语义等价（AC-11 契约面——对外键集与形状不回退）
// ---------------------------------------------------------------------------

describe('useExploreSession：返回形状语义等价（AC-11 契约面）', () => {
  it('renderHook 断言对外返回键集恰为八键且形状不回退（messages/chain/events/loading/running/error/send/stop）', async () => {
    const { result } = await mounted();

    // 键集整体断言：恰为契约八键、无缺漏无多余（任一键被移除或更名即失败）
    expect(Object.keys(result.current).sort()).toEqual(
      ['chain', 'error', 'events', 'loading', 'messages', 'running', 'send', 'stop'].sort(),
    );

    // 形状逐键断言：数据键为数组（messages 为 UIMessage 序列、chain 为轮统计行
    // 列表、events 为密封事件扁平镜像）、状态键为 boolean / string | null、
    // 行为键为函数（send / stop）
    expect(Array.isArray(result.current.messages)).toBe(true);
    expect(Array.isArray(result.current.chain)).toBe(true);
    expect(Array.isArray(result.current.events)).toBe(true);
    expect(typeof result.current.loading).toBe('boolean');
    expect(typeof result.current.running).toBe('boolean');
    expect(result.current.error === null || typeof result.current.error === 'string').toBe(true);
    expect(typeof result.current.send).toBe('function');
    expect(typeof result.current.stop).toBe('function');

    // 装载完成后的初值形状（既有会话全史转录重放落地；内容面由重放/续话
    // 专项用例承载，此处仅锚定非空与行为键可调用）
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.chain.length).toBeGreaterThan(0);
    expect(result.current.messages.length).toBeGreaterThan(0);
    expect(result.current.send).toBeInstanceOf(Function);
    expect(result.current.stop).toBeInstanceOf(Function);
  });
});
