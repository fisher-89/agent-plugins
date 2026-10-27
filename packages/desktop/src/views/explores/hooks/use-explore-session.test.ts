import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { buildExplorePrompt } from '../../../lib/explore-stance';
import { driftCount, driftReports, resetDrift } from '../../../lib/ipc-drift';
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

    expect(result.current.session.loading).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.session.chain).toEqual(sessionFixture.turns);
    expect(result.current.session.events).toEqual(transcript as AgentEvent[]);
    expect(result.current.session.error).toBeNull();
  });

  it('record 为 null：不还原会话（events 空态、loading 复位），send 为 no-op', async () => {
    const { result } = await mounted(null);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.session.chain).toEqual([]);
    expect(result.current.session.events).toEqual([]);
    expect(result.current.session.loading).toBe(false);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.session.running).toBe(false);
  });

  it('agent_session_transcript reject：错误可见不崩（error 置位）', async () => {
    transcript = '转录查询失败';

    const { result } = await mounted();
    await waitFor(() => expect(result.current.session.error).toContain('转录查询失败'));

    expect(result.current.session.loading).toBe(false);
  });
});

describe('useExploreSession：send 拼接 stance 与当前会话续话（AC-9，D4/D7）', () => {
  it('有会话时 send（续话）：prompt 裸发送（不重复携带 stance 前导）、sessionId 为当前会话、三元组齐全', async () => {
    const { result } = await mounted();
    expect(result.current.session.chain).toHaveLength(2);

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
    expect(result.current.session.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    // 提前 resolve：running 轮行入链（轮 id 立即可用，会话镜像回写）
    expect(result.current.session.chain).toHaveLength(3);
    expect(result.current.session.chain[2]).toEqual(turn(13, 'running', 'ses-13-1727000001000'));

    // 终态轮行经 Channel 信封回流（后端闭流收尾）：收口以库内重放收敛
    sessionsResult = [
      sessionSummary([turn(11), turn(12), turn(13, 'completed', 'ses-13-1727000001000')]),
    ];
    deliverRecord(turn(13, 'completed', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.session.running).toBe(false));
    expect(result.current.session.chain[2]).toEqual(turn(13, 'completed', 'ses-13-1727000001000'));

    act(() => {
      result.current.send({ ...SEND_INPUT, prompt: '再来一轮' });
    });
    await act(async () => {});

    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(2);
    // 收口重放后镜像归一会话行 id（库为准）：续话寻址取重放行而非运行期轮行
    expect(startCallArgs().sessionId).toBe(SESSION_ID);
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
    // 提前 resolve 的 running 轮行即库态 running 来源（在挂流门闩的旁证）
    const { result } = await mounted();
    expect(result.current.session.chain).toHaveLength(2);

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.session.running).toBe(true));

    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);

    // 终态轮行信封回流 → running 复位（收口以库内重放收敛）
    sessionsResult = [
      sessionSummary([turn(11), turn(12), turn(13, 'stopped', 'ses-13-1727000001000')]),
    ];
    deliverRecord(turn(13, 'stopped', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.session.running).toBe(false));
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

    expect(result.current.session.events).toContainEqual(textEvent(0, '实时片段'));
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
    await waitFor(() => expect(result.current.session.running).toBe(true));

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
    expect(result.current.session.events).toContainEqual(textEvent(0, '停止前已产出'));

    // 终态轮行回流：running 经 Record 复位（收口按库内重放行呈现）
    sessionsResult = [
      sessionSummary([turn(11), turn(12), turn(13, 'stopped', 'ses-13-1727000001000')]),
    ];
    deliverRecord(turn(13, 'stopped', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.session.running).toBe(false));
    expect(result.current.session.chain.at(-1)?.status).toBe('stopped');
  });
});

describe('useExploreSession：失败语义（AC-9）', () => {
  it('agent_start reject：error 置位、running 复位、既有事件不丢', async () => {
    const { result } = await mounted();
    const eventsBefore = [...result.current.session.events];
    expect(eventsBefore.length).toBeGreaterThan(0);

    startResult = '启动失败';
    act(() => {
      result.current.send(SEND_INPUT);
    });

    await waitFor(() => expect(result.current.session.running).toBe(false));
    expect(result.current.session.error).toContain('启动失败');
    expect(result.current.session.events).toEqual(eventsBefore);
    expect(result.current.session.chain).toEqual(sessionFixture.turns);
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

    expect(result.current.session.loading).toBe(true);
    expect(result.current.session.chain).toEqual([]);
    expect(result.current.session.events).toEqual([]);
    expect(result.current.session.error).toBeNull();
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
    expect(result.current.session.running).toBe(false);
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
    expect(result.current.session.chain).toEqual(sessionB.turns);

    await act(async () => {
      resolveA(sessionFixture);
    });
    expect(result.current.session.chain).toEqual(sessionB.turns);
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
    expect(result.current.session.error).toBeNull();
    expect(result.current.session.chain).toEqual(sessionB.turns);
  });
});

// ---------------------------------------------------------------------------
// 补遗：返回形状语义等价（AC-11 契约面——对外键集与形状不回退）
// ---------------------------------------------------------------------------

describe('useExploreSession：返回形状语义等价（AC-11 契约面）', () => {
  it('renderHook 断言对外返回键集恰为四键且形状不回退（session/sessionId/send/stop）', async () => {
    const { result } = await mounted();

    // 键集整体断言：恰为契约四键、无缺漏无多余（会话状态面经 `session` 承载）
    expect(Object.keys(result.current).sort()).toEqual(
      ['session', 'sessionId', 'send', 'stop'].sort(),
    );

    // 形状逐键断言：数据键为数组（messages 为 UIMessage 序列、chain 为轮统计行
    // 列表、events 为密封事件扁平镜像）、状态键为 boolean / string | null、
    // 行为键为函数（send / stop）
    expect(Array.isArray(result.current.session.messages)).toBe(true);
    expect(Array.isArray(result.current.session.chain)).toBe(true);
    expect(Array.isArray(result.current.session.events)).toBe(true);
    expect(typeof result.current.session.loading).toBe('boolean');
    expect(typeof result.current.session.running).toBe('boolean');
    expect(
      result.current.session.error === null || typeof result.current.session.error === 'string',
    ).toBe(true);
    expect(typeof result.current.send).toBe('function');
    expect(typeof result.current.stop).toBe('function');

    // 装载完成后的初值形状（既有会话全史转录重放落地；内容面由重放/续话
    // 专项用例承载，此处仅锚定非空与行为键可调用）
    expect(result.current.session.loading).toBe(false);
    expect(result.current.session.error).toBeNull();
    expect(result.current.session.chain.length).toBeGreaterThan(0);
    expect(result.current.session.messages.length).toBeGreaterThan(0);
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
    // 已定名记录 → 笔记定位形（插值笔记路径，AC-8 stance 分形）
    expect(String(args.prompt)).toContain('openspec/explores/api-retry.md');
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
    expect(result.current.session.chain).toHaveLength(2);

    // 既有会话（api-retry 全史）续话：裸发送
    act(() => {
      result.current.send(SEND_INPUT);
    });
    await act(async () => {});
    expect(String(startCallArgs().prompt)).toBe(SEND_INPUT.prompt);

    // 轮终态回流闭流，解除 running 门闩
    deliverRecord(turn(13, 'completed', 'ses-13-1727000001000'));
    await waitFor(() => expect(result.current.session.running).toBe(false));

    // 换记录（sessionId 空缺 → ensure 归位出空会话）：镜像重放清空 → 首轮
    // stance 随新 name 注入，sessionId 沿用归位会话（不新铸）
    rerender({ record: { ...exploreRecord(), id: 8, name: 'other-topic', sessionId: null } });
    await act(async () => {});
    await waitFor(() => expect(result.current.session.chain).toEqual([]));

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

    await waitFor(() => expect(result.current.session.chain).toEqual(sessionFixture.turns));
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

    await waitFor(() => expect(result.current.session.chain).toHaveLength(2));
    expect(
      invokeMock.mock.calls.filter(([name]) => name === 'ensure_explore_session'),
    ).toHaveLength(0);
  });

  it('归位失败：静默降级——无会话可重放、send 为 no-op（不新铸会话）', async () => {
    mockEnsure('归位失败');

    const { result } = await mounted(missingSession);
    await act(async () => {});

    expect(result.current.session.chain).toEqual([]);
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

    expect(result.current.session.loading).toBe(false);
    expect(result.current.session.error).toBeNull();
    expect(result.current.session.events).toHaveLength(3);

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

    await waitFor(() => expect(result.current.session.loading).toBe(false));
    expect(result.current.session.error).toBeNull();
    expect(sdkSession.row.configSnapshot).toEqual({
      engine: 'sdk',
      agentInstanceId: 9,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    });
  });
});

// ---------------------------------------------------------------------------
// auto-named-explore：auto-send 闸门（第三参 autoPrompt；四条件 + per-mount
// 一次性 ref）——既有返回面零变化
// ---------------------------------------------------------------------------

const AUTO_PROMPT = '调研 API 重试策略在超时边界的失效形态';
const DRAFT_NAME = 'draft-1758123456789';

/** 携 autoPrompt 第三参的挂载底座（空会话形态：建档预建空会话首轮）。 */
async function mountedWithAutoPrompt(
  record: ExploreRecord | null = exploreRecord(),
  autoPrompt: string | null = AUTO_PROMPT,
) {
  const rendered = renderHook(
    (props: { record: ExploreRecord | null; autoPrompt: string | null }) =>
      useExploreSession(ROOT, props.record, props.autoPrompt),
    { initialProps: { record, autoPrompt } },
  );
  await act(async () => {});
  return rendered;
}

/** 空会话基线（零轮零转录——auto-send 闸门的 chain === 0 前提）。 */
function mockEmptySession() {
  sessionsResult = [];
  transcript = [];
  startResult = turn(13, 'running', 'ses-13-1727000001000');
}

const startCallCount = () =>
  invokeMock.mock.calls.filter(([name]) => name === 'agent_start').length;

describe('useExploreSession：auto-send 闸门（AC-1）', () => {
  it('正向：闸门齐备（autoPrompt 在场 + 装载完成 + 链空 + 记录/会话就绪）→ agent_start 恰一次、prompt = stance 前导 + 需求原文', async () => {
    mockEmptySession();
    const { result } = await mountedWithAutoPrompt();

    await waitFor(() => expect(startCallCount()).toBe(1));
    const args = startCallArgs();
    expect(args.prompt).toBe(buildExplorePrompt(AUTO_PROMPT, 'api-retry'));
    expect(String(args.prompt)).toContain(AUTO_PROMPT);
    expect(String(args.prompt).startsWith('你在探索模式下工作')).toBe(true);
    // early-resolve running 轮行入链（恰一次发送的链路面佐证）
    await waitFor(() => expect(result.current.session.chain).toHaveLength(1));
  });

  it('档位口径：auto-send 的 agent_start 入参 permissionMode = bypassPermissions（与 composer 缺省档同源）', async () => {
    mockEmptySession();
    await mountedWithAutoPrompt();

    await waitFor(() => expect(startCallCount()).toBe(1));
    expect(startCallArgs().permissionMode).toBe('bypassPermissions');
    expect(startCallArgs().source).toBe('explore');
  });

  it('正向：autoPrompt 为 null（未传 / 显式传 null）→ 零自动发送（既有行为回归）', async () => {
    mockEmptySession();
    await mountedWithAutoPrompt(exploreRecord(), null);
    await act(async () => {});
    expect(startCallCount()).toBe(0);

    // 二参形态（缺省第三参）同口径
    const rendered = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: exploreRecord() } },
    );
    await act(async () => {});
    expect(rendered.result.current.session.chain).toEqual([]);
    expect(startCallCount()).toBe(0);
  });

  it('边界：chain > 0（既有会话重放完成）→ 不重发（back/forward 闸门）', async () => {
    // 缺省 fixture：两轮既有会话 → 装载完成后 chain 非空
    const { result } = await mountedWithAutoPrompt();

    await waitFor(() => expect(result.current.session.chain).toHaveLength(2));
    await act(async () => {});
    expect(startCallCount()).toBe(0);
  });

  it('边界：loading 在途 → 装载完成后才触发（不早发、不漏发、恰一次）', async () => {
    mockEmptySession();
    let resolveDetail: ((value: SessionSummary | null) => void) | null = null;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'session_detail') {
        return new Promise<SessionSummary | null>((resolve) => {
          resolveDetail = resolve;
        });
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      if (command === 'agent_start') {
        return typeof startResult === 'string'
          ? Promise.reject(startResult)
          : Promise.resolve(startResult);
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(
      (props: { record: ExploreRecord | null; autoPrompt: string | null }) =>
        useExploreSession(ROOT, props.record, props.autoPrompt),
      { initialProps: { record: exploreRecord(), autoPrompt: AUTO_PROMPT } },
    );
    await act(async () => {});

    // 装载在途：不早发
    expect(result.current.session.loading).toBe(true);
    expect(startCallCount()).toBe(0);

    await act(async () => {
      resolveDetail?.(null);
    });
    // 装载完成：不漏发、恰一次
    await waitFor(() => expect(startCallCount()).toBe(1));
    expect(result.current.session.loading).toBe(false);
  });

  it('边界：record 为 null / sessionId 空缺且归位失败 → 不自动发送', async () => {
    mockEmptySession();
    // record = null
    await mountedWithAutoPrompt(null);
    await act(async () => {});
    expect(startCallCount()).toBe(0);

    // sessionId 空缺 + ensure reject（会话未就绪）
    invokeMock.mockImplementation((command: string) => {
      if (command === 'ensure_explore_session') return Promise.reject('归位失败');
      if (command === 'session_detail') return Promise.resolve(null);
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    await mountedWithAutoPrompt({ ...exploreRecord(), sessionId: null });
    await act(async () => {});
    expect(startCallCount()).toBe(0);
  });

  it('边界：同 mount 内 effect 重复执行（StrictMode 双调用形态）→ send 恰一次（一次性消费 ref）', async () => {
    mockEmptySession();
    const { result, rerender } = await mountedWithAutoPrompt();

    await waitFor(() => expect(startCallCount()).toBe(1));
    // 重渲染（同记录同 autoPrompt）：effect 重跑但 ref 已消费
    rerender({ record: exploreRecord(), autoPrompt: AUTO_PROMPT });
    await act(async () => {});
    expect(startCallCount()).toBe(1);
    expect(result.current.session.chain).toHaveLength(1);
  });

  it('边界：unmount 后携带 state 重挂载 → ref 重新武装、可再次 auto-send（免费重试语义）', async () => {
    mockEmptySession();
    const first = await mountedWithAutoPrompt();
    await waitFor(() => expect(startCallCount()).toBe(1));
    first.unmount();

    const second = await mountedWithAutoPrompt();
    await waitFor(() => expect(startCallCount()).toBe(2));
    await waitFor(() => expect(second.result.current.session.chain).toHaveLength(1));
  });

  it('边界：首轮 send 失败（chain 仍 0）+ 重挂载复现 state → 恰构成一次重发（proposal 风险 6 留痕口径）', async () => {
    mockEmptySession();
    startResult = '启动失败';
    const failed = await mountedWithAutoPrompt();
    await waitFor(() => expect(failed.result.current.session.error).toContain('启动失败'));
    expect(startCallCount()).toBe(1);
    expect(failed.result.current.session.chain).toHaveLength(0);
    failed.unmount();

    // 重挂载（back/forward 复现 location state 的生产形态）：ref 重武装 → 重发
    startResult = turn(13, 'running', 'ses-13-1727000001000');
    const retried = await mountedWithAutoPrompt();
    await waitFor(() => expect(startCallCount()).toBe(2));
    await waitFor(() => expect(retried.result.current.session.chain).toHaveLength(1));
  });

  it('边界：autoPrompt 空串（非 null）→ 按在场口径发首轮（与 null 的区分断言）', async () => {
    mockEmptySession();
    const { result } = await mountedWithAutoPrompt(exploreRecord(), '');

    await waitFor(() => expect(startCallCount()).toBe(1));
    expect(startCallArgs().prompt).toBe(buildExplorePrompt('', 'api-retry'));
    await waitFor(() => expect(result.current.session.chain).toHaveLength(1));
  });
});

describe('useExploreSession：auto-send stance 分形（AC-1 / AC-8）', () => {
  it('占位记录（draft- 前缀）：send prompt 为命名指令形（不含任何文件路径插值）', async () => {
    mockEmptySession();
    const draft = { ...exploreRecord(), name: DRAFT_NAME };
    await mountedWithAutoPrompt(draft);

    await waitFor(() => expect(startCallCount()).toBe(1));
    const prompt = String(startCallArgs().prompt);
    expect(prompt).toBe(buildExplorePrompt(AUTO_PROMPT, DRAFT_NAME));
    expect(prompt).toContain('## 笔记定名与落盘');
    expect(prompt).not.toContain('openspec/explores');
  });

  it('已定名记录：send prompt 为笔记定位形（插值 record.name 与 openspec/explores/<name>.md）', async () => {
    mockEmptySession();
    await mountedWithAutoPrompt();

    await waitFor(() => expect(startCallCount()).toBe(1));
    const prompt = String(startCallArgs().prompt);
    expect(prompt).toBe(buildExplorePrompt(AUTO_PROMPT, 'api-retry'));
    expect(prompt).toContain('## 笔记定位');
    expect(prompt).toContain('openspec/explores/api-retry.md');
  });
});

describe('useExploreSession：auto-send 失败语义', () => {
  it('agent_start reject：既有 error 面承接、链路可续（失败不破坏 hook 状态）', async () => {
    mockEmptySession();
    startResult = '首轮启动失败';
    const { result } = await mountedWithAutoPrompt();

    await waitFor(() => expect(result.current.session.error).toContain('首轮启动失败'));
    expect(result.current.session.running).toBe(false);
    expect(result.current.session.chain).toHaveLength(0);

    // 链路可续：手动 send 可再次发起（不因失败卡死）
    act(() => {
      result.current.send({ prompt: '重试首轮', permissionMode: 'bypassPermissions' });
    });
    await act(async () => {});
    expect(startCallCount()).toBe(2);
    expect(String(startCallArgs().prompt)).toContain('重试首轮');
  });
});

// ---------------------------------------------------------------------------
// 补杀：ensure 归位守卫与取消抑制（root 缺失 / 记录切换竞态）
// ---------------------------------------------------------------------------

describe('useExploreSession：ensure 归位守卫与取消抑制（AC-9）', () => {
  it('root 为 null 且记录会话空缺：ensure 早退零 invoke（send 未就绪态）', async () => {
    const noSession: ExploreRecord = { ...exploreRecord(), sessionId: null };

    renderHook(() => useExploreSession(null, noSession));
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('记录切换：旧记录迟到的 ensure resolve 不落到新记录会话（取消抑制），新记录归位照常生效', async () => {
    const recordA: ExploreRecord = { ...exploreRecord(), sessionId: null };
    const recordB: ExploreRecord = {
      ...exploreRecord(),
      id: 8,
      name: 'other-topic',
      sessionId: null,
    };
    const pending = new Map<string, (value: ExploreRecord) => void>();
    invokeMock.mockImplementation((command: string, params: Record<string, string> = {}) => {
      if (command === 'ensure_explore_session') {
        return new Promise<ExploreRecord>((resolve) => {
          pending.set(String(params.name), resolve);
        });
      }
      if (command === 'session_detail') return Promise.resolve(null);
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (props: { record: ExploreRecord | null }) => useExploreSession(ROOT, props.record),
      { initialProps: { record: recordA } },
    );
    await act(async () => {});
    expect(pending.has('api-retry')).toBe(true);

    rerender({ record: recordB });
    await act(async () => {});
    expect(pending.has('other-topic')).toBe(true);

    // 旧记录迟到应答：会话 id 不得落到当前记录（否则以旧会话重放）
    await act(async () => {
      pending.get('api-retry')?.({ ...recordA, sessionId: 'ses-stale-1727000009000' });
    });
    await act(async () => {});

    const sessionDetailIds = () =>
      invokeMock.mock.calls
        .filter(([name]) => name === 'session_detail')
        .map(([, params]) => (params as { sessionId?: string }).sessionId);
    expect(sessionDetailIds()).not.toContain('ses-stale-1727000009000');
    expect(result.current.session.chain).toEqual([]);

    // 正面对照：当前记录的 ensure 归位照常生效（以新会话发起重放）
    await act(async () => {
      pending.get('other-topic')?.({ ...recordB, sessionId: 'ses-b-1727000002000' });
    });
    await waitFor(() => expect(sessionDetailIds()).toContain('ses-b-1727000002000'));
  });
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`ensureExploreSession`
// 的参数面放行与结果面 soft-report（记录缺字段时既有会话绑定不崩）。
// ---------------------------------------------------------------------------

describe('useExploreSession：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('ensureExploreSession(root, recordName) 放行：更新后记录干净零漂移、既有会话绑定不变', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'ensure_explore_session'
        ? Promise.resolve(exploreRecord())
        : Promise.resolve(null),
    );
    const { result } = renderHook(() =>
      useExploreSession(ROOT, { ...exploreRecord(), sessionId: null }),
    );
    await waitFor(() => expect(result.current.sessionId).toBe(SESSION_ID));

    expect(invokeMock.mock.calls.some(([name]) => name === 'ensure_explore_session')).toBe(true);
    expect(result.current.sessionId).toBe(SESSION_ID);
    expect(driftCount()).toBe(0);
  });

  it('结果含 null 可空字段 → 零漂移、既有状态映射不变', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'ensure_explore_session'
        ? Promise.resolve({ ...exploreRecord(), promotedTo: null, sessionId: null })
        : Promise.resolve(null),
    );
    renderHook(() => useExploreSession(ROOT, { ...exploreRecord(), sessionId: null }));
    await act(async () => {});

    expect(driftCount()).toBe(0);
  });

  it('reject 串 → 既有静默语义（不置 error 态）、零漂移', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'ensure_explore_session'
        ? Promise.reject('ensure 失败：record 缺失')
        : Promise.resolve(null),
    );
    const { result } = renderHook(() =>
      useExploreSession(ROOT, { ...exploreRecord(), sessionId: null }),
    );
    await act(async () => {});

    expect(result.current.session.error).toBeNull();
    expect(driftCount()).toBe(0);
  });

  it('结果面契约外（记录缺 name）→ 漂移恰 1 条、会话状态不崩', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'ensure_explore_session'
        ? Promise.resolve({ id: RECORD_ID, root: ROOT, sessionId: SESSION_ID })
        : Promise.resolve(null),
    );
    const { result } = renderHook(() =>
      useExploreSession(ROOT, { ...exploreRecord(), sessionId: null }),
    );
    await act(async () => {});

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('ensureExploreSession');
    expect(result.current.session.loading).toBe(false);
  });

  it('已有会话（幂等命中）重复进入 → 放行、零漂移、零 ensure 调用', async () => {
    invokeMock.mockResolvedValue(null);
    const { result } = renderHook(() => useExploreSession(ROOT, exploreRecord()));
    await act(async () => {});

    expect(invokeMock.mock.calls.some(([name]) => name === 'ensure_explore_session')).toBe(false);
    expect(result.current.sessionId).toBe(SESSION_ID);
    expect(driftCount()).toBe(0);
  });

  it('recordName 超长（> 1000 字符）→ 参数面放行（schema 只表达接受面），后端判定', async () => {
    const longName = 'x'.repeat(1200);
    invokeMock.mockImplementation((command: string) =>
      command === 'ensure_explore_session'
        ? Promise.resolve({ ...exploreRecord(), name: longName, sessionId: SESSION_ID })
        : Promise.resolve(null),
    );
    renderHook(() =>
      useExploreSession(ROOT, { ...exploreRecord(), name: longName, sessionId: null }),
    );
    await act(async () => {});

    expect(driftCount()).toBe(0);
  });
});
