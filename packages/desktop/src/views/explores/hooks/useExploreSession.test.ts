import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { buildExplorePrompt } from '../../../lib/exploreStance';
import type { AgentEvent, AgentRunRecord, ExploreRecord } from '../../../types/dto';
import { useExploreSession } from './useExploreSession';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_run_chain / agent_run_events /
// agent_start 记录入参供断言、可切换 reject）；Channel mock 为可编程 class
// （捕获 onmessage）。stance 前导不 mock：buildExplorePrompt 以真实实现参与
// send 拼接语义断言。
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

function exploreRecord(): ExploreRecord {
  return {
    id: RECORD_ID,
    root: ROOT,
    name: 'api-retry',
    createdAt: 1727000000000,
    updatedAt: 1727000000000,
  };
}

function run(id: number, sessionId: string | null, parentRunId: number | null): AgentRunRecord {
  return {
    id,
    prompt: `explore 轮次 ${id}`,
    cwd: ROOT,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status: 'completed',
    startedAt: 1727000000000 + id,
    finishedAt: 1727000001000 + id,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    sessionId,
    error: null,
    source: 'explore',
    sourceRef: String(RECORD_ID),
    parentRunId,
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

function runResultEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runResult',
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

/** 链 fixture：两条 parent_run_id 相连的 run（链头在前）。 */
const chainFixture = [run(11, 's-first', null), run(12, 's-tail', 11)];

let chainResult: AgentRunRecord[] | string;
let eventsByRun: Map<number, AgentEvent[]> | string;
let startResult: AgentRunRecord | string | null;

function mockIpc() {
  ChannelMock.instances.length = 0;
  chainResult = chainFixture.map((item) => ({ ...item }));
  eventsByRun = new Map<number, AgentEvent[]>([
    [11, [textEvent(0, '首轮结论')]],
    [12, [textEvent(0, '续轮结论'), runResultEvent(1)]],
  ]);
  startResult = run(13, 's-new-tail', 12);
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'agent_run_chain') {
      expect(params).toMatchObject({ source: 'explore', sourceRef: String(RECORD_ID) });
      return typeof chainResult === 'string'
        ? Promise.reject(chainResult)
        : Promise.resolve(chainResult);
    }
    if (command === 'agent_run_events') {
      if (typeof eventsByRun === 'string') {
        return Promise.reject(eventsByRun);
      }
      const runId = Number(params?.runId);
      return Promise.resolve(eventsByRun.get(runId) ?? []);
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
  env: 'default' as const,
  permissionMode: 'bypassPermissions' as const,
};

describe('useExploreSession：链还原与逐 run 事件重放（AC-9/AC-5）', () => {
  it('record 有既有链：挂载发起 agent_run_chain 恰一次（来源二元组）+ 逐 run 重放，events 按发起序拼接', async () => {
    const { result } = await mounted();

    expect(result.current.loading).toBe(false);
    const chainCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_run_chain');
    expect(chainCalls).toHaveLength(1);
    expect(chainCalls[0]).toEqual([
      'agent_run_chain',
      { source: 'explore', sourceRef: String(RECORD_ID) },
    ]);
    expect(result.current.chain).toEqual(chainFixture);
    expect(result.current.events).toEqual([
      textEvent(0, '首轮结论'),
      textEvent(0, '续轮结论'),
      runResultEvent(1),
    ]);
    expect(result.current.error).toBeNull();
  });

  it('record 为 null：不还原链（events 空态、loading 复位），send 为 no-op', async () => {
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

  it('agent_run_events 对中途 run reject：错误可见不崩（error 置位）', async () => {
    eventsByRun = '事件查询失败';

    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toContain('事件查询失败'));

    expect(result.current.loading).toBe(false);
  });
});

describe('useExploreSession：send 拼接 stance 与链尾 resume（AC-9，D4/D7）', () => {
  it('有链时 send：prompt 以 stance 前导开头、resume 为链尾 sessionId、三元组齐全', async () => {
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
    expect(args.resumeSessionId).toBe('s-tail');
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBe(String(RECORD_ID));
    expect(args.parentRunId).toBe(12);
    expect(args.root).toBe(ROOT);
    expect(args.env).toBe('default');
    expect(args.permissionMode).toBe('bypassPermissions');
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('无链时 send（首条起链）：resume 与 parent 不传、来源三元组仍携带', async () => {
    chainResult = [];
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.resumeSessionId).toBeNull();
    expect(args.parentRunId).toBeNull();
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBe(String(RECORD_ID));
  });

  it('run 终态后：链增长，二次 send 的 resume 更新为新链尾 sessionId', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(result.current.chain).toHaveLength(3);
    expect(result.current.chain[2]).toEqual(run(13, 's-new-tail', 12));

    act(() => {
      result.current.send({ ...SEND_INPUT, prompt: '再来一轮' });
    });
    await act(async () => {});

    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(2);
    expect(startCallArgs().resumeSessionId).toBe('s-new-tail');
    expect(startCallArgs().parentRunId).toBe(13);
  });

  it('链中某 run 无 session_id（异常终态）：send 不携带 resume 而非传空串', async () => {
    chainResult = [run(11, null, null)];
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.resumeSessionId).toBeNull();
    expect(args.parentRunId).toBe(11);
  });

  it('运行中重复 send：被抑制（running 门闩），不并发发起第二条 run', async () => {
    let resolveStart: ((value: AgentRunRecord) => void) | null = null;
    startResult = null;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') {
        return new Promise<AgentRunRecord>((resolve) => {
          resolveStart = resolve;
        });
      }
      if (command === 'agent_run_chain') {
        return Promise.resolve(chainFixture);
      }
      if (command === 'agent_run_events') {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    expect(result.current.running).toBe(true);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);

    act(() => {
      resolveStart?.(run(13, 's-new-tail', 12));
    });
    await waitFor(() => expect(result.current.running).toBe(false));
  });

  it('send 成功后实时 Channel 事件累积进 events', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    act(() => {
      lastChannel().onmessage?.(textEvent(0, '实时片段'));
    });
    await act(async () => {});

    expect(result.current.events).toContainEqual(textEvent(0, '实时片段'));
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
    expect(result.current.chain).toEqual(chainFixture);
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：加载在途态、root 守卫、记录切换竞态（取消抑制）。
// ---------------------------------------------------------------------------

describe('useExploreSession：加载在途态与守卫（AC-9）', () => {
  it('链查询在途：chain / events 保持初始空态、loading 置位', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_run_chain') return new Promise<AgentRunRecord[]>(() => {});
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
  const chainB = [run(21, 's-b', null)];

  it('旧记录迟到的链 resolve 不覆盖新链', async () => {
    let resolveA: (value: AgentRunRecord[]) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'agent_run_chain') {
        if (params?.sourceRef === '7') {
          return new Promise<AgentRunRecord[]>((resolve) => {
            resolveA = resolve;
          });
        }
        return Promise.resolve(chainB);
      }
      if (command === 'agent_run_events') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});

    rerender({ record: recordB });
    await act(async () => {});
    expect(result.current.chain).toEqual(chainB);

    await act(async () => {
      resolveA(chainFixture);
    });
    expect(result.current.chain).toEqual(chainB);
  });

  it('旧记录迟到的链 reject 不置错误、不污染新链', async () => {
    let rejectA: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'agent_run_chain') {
        if (params?.sourceRef === '7') {
          return new Promise<AgentRunRecord[]>((_resolve, reject) => {
            rejectA = reject;
          });
        }
        return Promise.resolve(chainB);
      }
      if (command === 'agent_run_events') return Promise.resolve([]);
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
      rejectA(new Error('迟到的链失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.chain).toEqual(chainB);
  });
});
