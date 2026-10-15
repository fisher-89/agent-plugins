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
    title: 'api-retry',
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: SESSION_ID,
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

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function sessionSummary(turns: TurnSummary[], id: string = SESSION_ID): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: 's-tail',
      configSnapshot: {
        engine: 'cli',
        agentInstanceId: null,
        model: null,
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
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
/** ensure_explore_session 应答（会话归位：返回补全 session_id 的记录） */
let ensureResult: ExploreRecord | string;

function mockIpc() {
  ChannelMock.instances.length = 0;
  sessionsResult = [sessionSummary([turn(11), turn(12)])];
  transcript = [textEvent(0, '首轮结论'), turnDoneEvent(1), textEvent(2, '续轮结论')];
  // 提前 resolve 契约：agent_start 返回 running 轮行（当前会话随 onRecord 确立）
  startResult = turn(13, 'running', 'ses-13-1727000001000');
  // 建档已预建空会话：ensure 幂等返回原记录（session_id 已在）
  ensureResult = exploreRecord();
  invokeMock.mockImplementation((command: string, _params?: Record<string, unknown>) => {
    if (command === 'session_detail') {
      return typeof sessionsResult === 'string'
        ? Promise.reject(sessionsResult)
        : Promise.resolve(sessionsResult[0] ?? null);
    }
    if (command === 'agent_session_transcript') {
      if (typeof transcript === 'string') {
        return Promise.reject(transcript);
      }
      return Promise.resolve(transcript);
    }
    if (command === 'ensure_explore_session') {
      return typeof ensureResult === 'string'
        ? Promise.reject(ensureResult)
        : Promise.resolve(ensureResult);
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
  it('record 有既有会话：挂载发起 session_detail 恰一次（携 root + sessionId）+ agent_session_transcript 全史重放，events 承接转录', async () => {
    const { result } = await mounted();

    expect(result.current.loading).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
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
  it('有会话时 send（续话）：prompt 裸发送（不重复携带 stance 前导）、sessionId 为当前会话、三元组齐全', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.prompt).toBe(SEND_INPUT.prompt);
    expect(args.sessionId).toBe(SESSION_ID);
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBeNull();
    expect(args.root).toBe(ROOT);
    expect(args.permissionMode).toBe('bypassPermissions');
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('无轮会话 send（预建空会话首轮）：prompt 拼 stance 前导、sessionId 沿用记录会话（Continue，不新铸）、来源三元组仍携带', async () => {
    // 建档预建空会话：sessionId 已在、零轮零转录——首轮即 Continue 开轮
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.prompt).toBe(buildExplorePrompt(SEND_INPUT.prompt, 'api-retry'));
    expect(args.sessionId).toBe(SESSION_ID);
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBeNull();
  });

  it('轮终态后：轮序列增长、终态轮行回流复位，二次 send 的 sessionId 更新为新轮行会话、prompt 裸发送', async () => {
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
    expect(startCallArgs().prompt).toBe('再来一轮');
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
      if (command === 'session_detail') {
        return Promise.resolve(sessionSummary([turn(11), turn(12)]));
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
      if (command === 'session_detail') return new Promise<SessionSummary>(() => {});
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
  // 记录切换以槽位 sessionId 为键（反查二元组退役）：recordB 携独立 sessionId，
  // recordA 保留 `SESSION_ID` 槽位；旧记录的迟到 resolve / reject 不覆盖新会话
  const recordB: ExploreRecord = {
    ...exploreRecord(),
    id: 8,
    name: 'other-topic',
    sessionId: 'ses-21-1727000000000',
  };
  const sessionB = sessionSummary([turn(21, 'completed', 'ses-21-1727000000000')]);

  it('旧记录迟到的会话 resolve 不覆盖新会话', async () => {
    let resolveA: (value: SessionSummary | null) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'session_detail') {
        if (params?.sessionId === SESSION_ID) {
          return new Promise<SessionSummary | null>((resolve) => {
            resolveA = resolve;
          });
        }
        return Promise.resolve(sessionB);
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
      resolveA(sessionFixture);
    });
    expect(result.current.chain).toEqual(sessionB.turns);
  });

  it('旧记录迟到的会话 reject 不置错误、不污染新会话', async () => {
    let rejectA: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'session_detail') {
        if (params?.sessionId === SESSION_ID) {
          return new Promise<SessionSummary | null>((_resolve, reject) => {
            rejectA = reject;
          });
        }
        return Promise.resolve(sessionB);
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

// ---------------------------------------------------------------------------
// send 注入 record.name（explore-name-file-binding AC-4 send 组装半边）：
// 仅新会话首条 prompt == buildExplorePrompt(input.prompt, record.name)（真实
// 实现参与拼接）；既有会话续话裸发送（不携带 stance / name）；换记录新会话
// 注入随新 name；record 为 null 不发送（零 IPC）。
// ---------------------------------------------------------------------------

describe('useExploreSession：send 注入 record.name（AC-4 组装半边）', () => {
  it('正向：新会话首条 send 的 prompt 逐字等于 buildExplorePrompt(input.prompt, record.name)', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args.prompt).toBe(buildExplorePrompt(SEND_INPUT.prompt, 'api-retry'));
    expect(String(args.prompt)).toContain('api-retry');
    expect(String(args.prompt)).toContain('文件名 MUST 等于 api-retry');
  });

  it('正向：既有会话续话裸发送（不携带 stance / name）；换记录（sessionId 空缺）经 ensure 归位后首轮随新 name 注入', async () => {
    // 槽位寻址（反查二元组退役）：recordA 携既有会话 sessionId 续话；recordB
    // 槽位空缺（存量行）→ ensure 归位出空会话 id，首轮（零轮）随新 name 拼 stance
    invokeMock.mockImplementation(
      (command: string, params: Record<string, string | number | null> = {}) => {
        if (command === 'session_detail') {
          return Promise.resolve(params.sessionId === SESSION_ID ? sessionFixture : null);
        }
        if (command === 'agent_session_transcript') return Promise.resolve([]);
        if (command === 'ensure_explore_session') {
          const name = String(params.name ?? '');
          return Promise.resolve({
            ...exploreRecord(),
            id: name === 'other-topic' ? 8 : RECORD_ID,
            name,
            sessionId: name === 'other-topic' ? 'ses-b-1727000002000' : SESSION_ID,
          } satisfies ExploreRecord);
        }
        if (command === 'agent_start') return Promise.resolve(startResult);
        return Promise.resolve(null);
      },
    );

    const { result, rerender } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});
    expect(result.current.chain).toHaveLength(2);

    // 既有会话（api-retry 全史）续话：裸发送
    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(String(startCallArgs().prompt)).toBe(SEND_INPUT.prompt);

    // 轮终态回流闭流，解除 running 门闩
    deliverRecord(turn(13, 'completed', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));

    // 换记录（sessionId 空缺 → ensure 归位出空会话）：镜像重放清空 → 首轮
    // stance 随新 name 注入，sessionId 沿用归位会话（不新铸）
    rerender({ record: { ...exploreRecord(), id: 8, name: 'other-topic', sessionId: null } });
    await act(async () => {});
    await waitFor(() => expect(result.current.chain).toEqual([]));

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(String(startCallArgs().prompt)).toBe(
      buildExplorePrompt(SEND_INPUT.prompt, 'other-topic'),
    );
    expect(startCallArgs().sessionId).toBe('ses-b-1727000002000');
  });

  it('边界：record 为 null → send 为 no-op（零 agent_start IPC）', async () => {
    const { result } = await mounted(null);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});

    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 会话归位（ensure_explore_session）：记录 session_id 空缺（存量 v3 升级行 /
// 写回缺失遗留）→ 挂载即 invoke 归位并承接其 id 作重放与续话键；保证「退出重
// 进详情页能看到会话」的修复半边。
// ---------------------------------------------------------------------------

describe('useExploreSession：会话归位 ensure 承接（session_id 空缺修复）', () => {
  const missingSession: ExploreRecord = { ...exploreRecord(), sessionId: null };
  const ensured: ExploreRecord = { ...exploreRecord(), sessionId: 'ses-adopt-1727000003000' };

  function mockEnsure(result: ExploreRecord | string) {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'ensure_explore_session') {
        return typeof result === 'string' ? Promise.reject(result) : Promise.resolve(result);
      }
      if (command === 'session_detail') return Promise.resolve(sessionFixture);
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      if (command === 'agent_start') return Promise.resolve(startResult);
      return Promise.resolve(null);
    });
  }

  it('record.sessionId 空缺：挂载即 ensure 归位，并以归位 id 重放（退出重进可还原会话）', async () => {
    mockEnsure(ensured);

    const { result } = await mounted(missingSession);

    await waitFor(() => expect(result.current.chain).toEqual(sessionFixture.turns));
    expect(invokeMock).toHaveBeenCalledWith('ensure_explore_session', {
      root: ROOT,
      name: 'api-retry',
    });
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: 'ses-adopt-1727000003000',
    });
  });

  it('record.sessionId 已在：不发起 ensure（建档已回写，幂等旁路）', async () => {
    mockEnsure(ensured);

    const { result } = await mounted();

    await waitFor(() => expect(result.current.chain).toHaveLength(2));
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'ensure_explore_session'),
    ).toHaveLength(0);
  });

  it('归位失败：静默降级——无会话可重放、send 为 no-op（不新铸会话）', async () => {
    mockEnsure('归位失败');

    const { result } = await mounted(missingSession);
    await act(async () => {});

    expect(result.current.chain).toEqual([]);
    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

describe('useExploreSession：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  it('补键后重放装载既有断言保绿（agentInstanceId null 存量缺列语义）', async () => {
    const { result } = await mounted();

    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(result.current.events).toHaveLength(3);

    const snapshot = sessionFixture.row.configSnapshot as ConfigSnapshotFixture;
    expect(snapshot).toHaveProperty('agentInstanceId', null);
    expect(snapshot.engine).toBe('cli');
    expect(snapshot.permissionMode).toBe('bypassPermissions');
  });

  it('sdk 新会话形态（agentInstanceId 具体 id）照常装载（两形态同口径）', async () => {
    const sdkSession = sessionSummary([turn(14)], 'ses-snap-2');
    sdkSession.row.configSnapshot = {
      engine: 'sdk',
      agentInstanceId: 9,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    } satisfies ConfigSnapshotFixture;
    sessionsResult = [sdkSession];
    transcript = [textEvent(0, '新会话输出')];

    const { result } = await mounted(exploreRecord());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeNull();
    expect(sdkSession.row.configSnapshot).toEqual({
      engine: 'sdk',
      agentInstanceId: 9,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    });
  });
});
