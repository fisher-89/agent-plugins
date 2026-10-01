import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { eventsToUIMessages, runRecordToUIMessage } from '../lib/agent-adapter';
import type { AgentEvent, SessionSummary, TurnSummary } from '../types/dto';
import { useAgentChat, type UseAgentChatParams } from './use-agent-chat';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_sessions / agent_session_transcript /
// agent_start / agent_stop，可切换 resolve/reject 并记录入参）；Channel mock
// 为可编程 class（捕获 onmessage，测试直接投递 Event / Record 信封驱动实时
// 流与终态回流）。适配层与 transport 以真实实现参与，不 mock `ai` 的 useChat。
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
const SOURCE_REF = '7';
const SESSION_ID = 'ses-3-1727000000000';

const DEBUG_PARAMS: UseAgentChatParams = { source: 'explore', sourceRef: SOURCE_REF, root: ROOT };

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

function sessionSummary(
  row: Partial<SessionSummary['row']> = {},
  turns: TurnSummary[],
): SessionSummary {
  return {
    row: {
      id: SESSION_ID,
      remoteSessionId: 's-engine',
      configSnapshot: { engine: 'sdk', model: 'glm-high', permissionMode: 'bypassPermissions' },
      provenance: { source: 'explore', sourceRef: SOURCE_REF },
      createdAt: 1727000000000,
      updatedAt: 1727000009999,
      ...row,
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

function turnDoneEvent(seq: number, sessionId: string = SESSION_ID): AgentEvent {
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
    sessionId,
  };
}

/** 会话 fixture：两轮往复（全史转录 4 事件 + 轮统计行 2 行，轮序对齐）。 */
const REPLAY_TURNS = [turn(11), turn(12)];
const REPLAY_EVENTS = [
  textEvent(0, '首轮结论'),
  turnDoneEvent(1),
  textEvent(2, '续轮结论'),
  turnDoneEvent(3),
];

const SEND_INPUT = {
  prompt: '继续追这条线索',
  permissionMode: 'bypassPermissions' as const,
};

// ---------------------------------------------------------------------------
// IPC 编排
// ---------------------------------------------------------------------------

let sessionsResult: SessionSummary[];
let transcript: AgentEvent[];
let startBehavior: { mode: 'resolve' | 'reject'; value: TurnSummary | string } | null = null;
let startCalls: number;

function mockIpc() {
  ChannelMock.instances.length = 0;
  sessionsResult = [sessionSummary({}, REPLAY_TURNS)];
  transcript = REPLAY_EVENTS.map((event) => ({ ...event }));
  startBehavior = null;
  startCalls = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_sessions') {
      return Promise.resolve(sessionsResult);
    }
    if (command === 'agent_session_transcript') {
      return Promise.resolve(transcript);
    }
    if (command === 'agent_start') {
      startCalls += 1;
      const behavior = startBehavior;
      if (behavior === null || behavior.mode === 'resolve') {
        return Promise.resolve(
          behavior === null ? turn(13, 'running', 'ses-9-1727000001000') : behavior.value,
        );
      }
      return Promise.reject(behavior.value);
    }
    if (command === 'agent_stop') {
      return Promise.resolve(null);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  mockIpc();
});

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('send 未创建 Channel（尚未发起 agent_start）');
  return instance;
}

function deliverEvent(event: AgentEvent) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'event', event });
  });
}

function deliverRecord(row: TurnSummary) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record: row });
  });
}

async function mounted(params: UseAgentChatParams = DEBUG_PARAMS) {
  const rendered = renderHook(
    (props: { params: UseAgentChatParams }) => useAgentChat(props.params),
    {
      initialProps: { params },
    },
  );
  await act(async () => {});
  return rendered;
}

function startCallArgs(): Record<string, unknown> {
  const call = invokeMock.mock.calls.filter(([name]) => name === 'agent_start').at(-1);
  if (!call) throw new Error('agent_start 未被调用');
  return call[1] as Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// 重放装载（最新会话 + 全史密封转录重放）
// ---------------------------------------------------------------------------

describe('useAgentChat：重放装载', () => {
  it('挂载发起 agent_sessions 携 (root, source, sourceRef) 三参 + agent_session_transcript 携 (root, sessionId)，经适配层重建消息；会话镜像与查询一致', async () => {
    const { result } = await mounted();

    const sessionCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
    expect(sessionCalls).toEqual([
      ['agent_sessions', { root: ROOT, source: 'explore', sourceRef: SOURCE_REF }],
    ]);
    expect(invokeMock).toHaveBeenCalledWith('agent_session_transcript', {
      root: ROOT,
      sessionId: SESSION_ID,
    });
    expect(result.current.session).toEqual(sessionSummary({}, REPLAY_TURNS));
    expect(result.current.chain).toEqual(REPLAY_TURNS);
    expect(result.current.events).toEqual(REPLAY_EVENTS);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    // 重建形状：密封转录折叠 + 轮统计行部件按 TurnDone 事件位交错（轮序对齐）
    expect(result.current.messages).toEqual([
      ...eventsToUIMessages([REPLAY_EVENTS[0], REPLAY_EVENTS[1]]),
      runRecordToUIMessage(REPLAY_TURNS[0]),
      ...eventsToUIMessages([REPLAY_EVENTS[2], REPLAY_EVENTS[3]]),
      runRecordToUIMessage(REPLAY_TURNS[1]),
    ]);
  });

  it('重放装载重建的 messages 与同批事件经 transport 实时流入 useChat 的终态形状一致（两路同构）', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));
    // 启动 resolve 的 running 轮行确立当前会话，密封事件入镜像
    act(() => {
      startBehavior = { mode: 'resolve', value: turn(13, 'running', 'ses-9-1727000001000') };
    });
    await act(async () => {});
    const realtimeEvents = [textEvent(0, '正文'), turnDoneEvent(1, 'ses-9-1727000001000')];
    for (const event of realtimeEvents) {
      deliverEvent(event);
    }
    const terminal = turn(13, 'completed', 'ses-9-1727000001000');
    deliverRecord(terminal);

    await waitFor(() => expect(result.current.running).toBe(false));
    // 收口归一后终态 = 折叠重建形状（重开恢复 = 重放的同构锚点）
    expect(result.current.messages).toEqual([
      ...eventsToUIMessages(realtimeEvents),
      runRecordToUIMessage(terminal),
    ]);
    expect(result.current.events).toEqual(realtimeEvents);
    expect(result.current.chain).toEqual([terminal]);
    expect(result.current.session?.row.id).toBe('ses-9-1727000001000');
  });
});

// ---------------------------------------------------------------------------
// 发送组装 / stop 接线 / 运行门闩
// ---------------------------------------------------------------------------

describe('useAgentChat：发送组装与停止', () => {
  it('sendMessage 经 body 携带当前会话 sessionId（Continue），三元组与 root 透传 agent_start', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args).toMatchObject({
      root: ROOT,
      prompt: SEND_INPUT.prompt,
      permissionMode: 'bypassPermissions',
      sessionId: SESSION_ID,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });
    expect(args['onEvent']).toBeInstanceOf(ChannelMock);
  });

  it('stop() 调 invoke("agent_stop", { root, sessionId }) 会话寻址且不截断前端流：Record 回流后 running 复位、消息含 record 部件', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));
    act(() => {
      startBehavior = { mode: 'resolve', value: turn(13, 'running', 'ses-9-1727000001000') };
    });
    await act(async () => {});
    deliverEvent(textEvent(0, '停止前已产出'));

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledWith('agent_stop', {
      root: ROOT,
      sessionId: 'ses-9-1727000001000',
    });

    // 前端流不被截断：stop 后终态轮行经 Channel 回流并复位 running
    const terminal = turn(13, 'stopped', 'ses-9-1727000001000');
    deliverRecord(terminal);
    await waitFor(() => expect(result.current.running).toBe(false));

    const stopCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_stop');
    expect(stopCalls).toHaveLength(1);
    expect(result.current.messages).toEqual([
      ...eventsToUIMessages([textEvent(0, '停止前已产出')]),
      runRecordToUIMessage(terminal),
    ]);
    expect(result.current.chain).toEqual([terminal]);
  });

  it('状态 submitted/streaming 期间重复 sendMessage 被忽略，不并发第二条轮', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    act(() => {
      result.current.sendMessage({ ...SEND_INPUT, prompt: '并发第二条' });
    });
    await act(async () => {});
    expect(startCalls).toBe(1);

    // 收尾解锁：终态轮行回流后恢复可发
    act(() => {
      startBehavior = { mode: 'resolve', value: turn(13, 'running', 'ses-9-1727000001000') };
    });
    await act(async () => {});
    deliverRecord(turn(13, 'completed', 'ses-9-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));
    expect(startCalls).toBe(1);
  });

  it('agent_start reject → error 置位、running 复位、既有 messages 不丢', async () => {
    const { result } = await mounted();
    const before = result.current.messages;
    expect(before.length).toBeGreaterThan(0);

    startBehavior = { mode: 'reject', value: '启动失败' };
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });

    await waitFor(() => expect(result.current.running).toBe(false));
    expect(result.current.error).toContain('启动失败');
    expect(result.current.messages).toEqual(before);
    expect(result.current.chain).toEqual(REPLAY_TURNS);
  });
});

// ---------------------------------------------------------------------------
// 边界：空会话 / null 守卫 / reset
// ---------------------------------------------------------------------------

describe('useAgentChat：边界', () => {
  it('无历史会话（空清单）→ 发送以 sessionId=null 穿透（New 语义），来源三元组仍齐全', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args['sessionId']).toBeNull();
    expect(args['source']).toBe('explore');
    expect(args['sourceRef']).toBe(SOURCE_REF);
  });

  it('root 为 null（未选定 workspace）：重放不发起、发送 no-op，不崩', async () => {
    const { result } = await mounted({ ...DEBUG_PARAMS, root: null });

    expect(invokeMock).not.toHaveBeenCalled();
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.running).toBe(false);
    expect(result.current.messages).toEqual([]);
  });

  it('sourceRef 为 null（debug 场景）：不重放装载，发送以 null 穿透 source / 会话参数，不崩', async () => {
    const { result } = await mounted({ source: 'debug', sourceRef: null, root: ROOT });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.messages).toEqual([]);

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args['source']).toBe('debug');
    expect(args['sourceRef']).toBeNull();
    expect(args['sessionId']).toBeNull();
  });

  it('reset() 清空 messages / events / chain / session 镜像（debug 页每跑重置场景）', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.reset();
    });

    expect(result.current.messages).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.chain).toEqual([]);
    expect(result.current.session).toBeNull();
    // 重置后可正常发起新一轮：空会话语义（sessionId 缺省 null 即 New）
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});
    expect(startCallArgs()['sessionId']).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 发送组装 agent 穿透：body 组装 `agent: input.agent ?? null`——缺席透传不
// 注入（缺省裁决权归后端解析默认 agent；debug 链显式传 id、explore 等正式链
// 缺席透传）
// ---------------------------------------------------------------------------

describe('useAgentChat：发送组装 agent 穿透', () => {
  it('sendMessage input.agent=7 → body.agent=7（invoke 入参断言）', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage({ ...SEND_INPUT, agent: 7 });
    });
    await act(async () => {});

    expect(startCallArgs()['agent']).toBe(7);
  });

  it('sendMessage input.agent=3 → body.agent=3（调试链显式传值穿透）', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage({ ...SEND_INPUT, agent: 3 });
    });
    await act(async () => {});

    expect(startCallArgs()['agent']).toBe(3);
  });

  it('input.agent 不传 → body.agent=null（缺席透传不注入，explore 链缺省安全）', async () => {
    // DEBUG_PARAMS 即 explore 形态（来源侧不传 agent）：explore 会话
    // 发送 body.agent=null → 后端缺省解析默认 agent
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    expect(startCallArgs()['agent']).toBeNull();
  });

  it('input.agent 为类型面外运行时异常值（yolo）→ hook 不做清单校验原样入 body，由 transport readAgentId 层拒绝（职责分界留痕）', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage({ ...SEND_INPUT, agent: 'yolo' as unknown as number });
    });
    await waitFor(() => expect(result.current.error).toContain('非法 agent'));

    // hook 本体不炸：error 置位、running 复位；拒绝发生在 transport 层
    //（invoke 未发起），hook 侧零清单校验代码路径
    expect(result.current.running).toBe(false);
    expect(result.current.error).toContain('非法 agent');
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(0);
  });

  it('agent 增补后 sessionId / 来源三元组组装不变（互不串线，body 逐字段断言）', async () => {
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage({ ...SEND_INPUT, agent: 7 });
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args['agent']).toBe(7);
    expect(args['sessionId']).toBe(SESSION_ID);
    expect(args['root']).toBe(ROOT);
    expect(args['source']).toBe('explore');
    expect(args['sourceRef']).toBe(SOURCE_REF);
    expect(args['prompt']).toBe(SEND_INPUT.prompt);
    expect(args['permissionMode']).toBe('bypassPermissions');
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：agent_sessions / agent_session_transcript /
// agent_stop 裸 invoke → typed bindings 机械替换后，invoke 命令名与 camelCase
// 参数逐字不变（生成绑定底层仍走 @tauri-apps/api/core 的 invoke，mock 机制
// 切换后依旧生效）；root 寻址参数随双库拆分进入会话查询与停止命令。
// ---------------------------------------------------------------------------

describe('useAgentChat：生成绑定调用面', () => {
  it('发起经生成绑定入口后 invoke 收到 "agent_start" 与 7 个 camelCase 会话参数逐字不变，返回 running 轮行透传', async () => {
    const { result } = await mounted();

    // 重放：命令名与 root 寻址三元组逐字不变
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    // 生成绑定位置参数 → invoke 参数对象：恰 8 个 key（会话参数 7 + onEvent）
    // （agent 键恒在——transport 恒传 agent，缺席值 null）
    const args = startCallArgs();
    expect(Object.keys(args).sort()).toEqual([
      'agent',
      'onEvent',
      'permissionMode',
      'prompt',
      'root',
      'sessionId',
      'source',
      'sourceRef',
    ]);
    expect(args).toMatchObject({
      root: ROOT,
      prompt: SEND_INPUT.prompt,
      permissionMode: 'bypassPermissions',
      agent: null,
      sessionId: SESSION_ID,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });
    // 返回 running 轮行透传（提前 resolve：running 置位、会话可寻址）
    await waitFor(() => expect(result.current.chain).toHaveLength(3));
    expect(result.current.running).toBe(true);
  });

  it('重放经生成绑定后 invoke 收到 "agent_session_transcript" 与 { root, sessionId }：无会话零转录调用、有会话恰一次全史调用', async () => {
    const { result } = await mounted();

    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_session_transcript')).toEqual([
      ['agent_session_transcript', { root: ROOT, sessionId: SESSION_ID }],
    ]);
    // 消息重建不变：4 密封事件 + 2 轮统计行部件 = 6 条
    expect(result.current.messages).toHaveLength(6);

    // 无会话：零转录调用（空会话语义在绑定切换后保持）
    const callsBeforeEmpty = invokeMock.mock.calls.length;
    sessionsResult = [];
    transcript = [];
    const empty = await mounted();
    expect(
      invokeMock.mock.calls
        .slice(callsBeforeEmpty)
        .filter(([name]) => name === 'agent_session_transcript'),
    ).toHaveLength(0);
    expect(empty.result.current.messages).toEqual([]);
  });

  it('停止经生成绑定后 invoke 收到 "agent_stop" 与 { root, sessionId }；reject 路径 error 置位 / running 复位行为不变', async () => {
    sessionsResult = [];
    transcript = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledWith('agent_stop', {
      root: ROOT,
      sessionId: 'ses-9-1727000001000',
    });

    // 终态轮行回流后 running 复位（停止路径行为不变）
    deliverRecord(turn(13, 'stopped', 'ses-9-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));

    // reject 路径：错误经生成绑定 reject 通道抵达，error 置位、running 复位
    act(() => {
      startBehavior = { mode: 'reject', value: 'CLI 未找到' };
    });
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.error).toContain('CLI 未找到'));
    expect(result.current.running).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 补遗：镜像密封-only（delta 只上 chunk 流）/ 装载双查询 reject 面（AC-8 /
// 行为不回退）
// ---------------------------------------------------------------------------

function textDeltaEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

describe('useAgentChat：镜像只收密封事件', () => {
  it('实时流投递 messageDelta：镜像不进 delta（仅 chunk 流可见），密封到达镜像更新——镜像与原始转录视图密封-only 同构', async () => {
    const { result } = await mounted();

    // 发起一轮以取得实时 Channel（提前 resolve running）
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});
    const beforeDelta = result.current.events;

    // delta 信封：镜像不进 delta（events 镜像原样）
    deliverEvent(textDeltaEvent(9, '实时增量'));
    await act(async () => {});
    expect(result.current.events).toEqual(beforeDelta);

    // 密封事件：镜像更新（append 保序）
    const sealed = textEvent(10, '密封结论');
    deliverEvent(sealed);
    await act(async () => {});
    expect(result.current.events).toEqual([...beforeDelta, sealed]);

    // 终态 record 收尾（running 复位、不悬挂）
    deliverRecord(turn(13, 'completed', 'ses-9-1727000001000'));
    await waitFor(() => expect(result.current.running).toBe(false));
  });
});

describe('useAgentChat：装载双查询 reject 面', () => {
  it('agent_sessions reject：error 态呈现（错误串透传）、loading 复位、不静默以空清单装载', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_sessions') return Promise.reject('db: 会话清单读取失败');
      return Promise.resolve(null);
    });

    const { result } = await mounted();

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('会话清单读取失败');
    expect(result.current.loading).toBe(false);
    // 未装载：镜像与消息保持空态（错误可见而非静默空）
    expect(result.current.events).toEqual([]);
    expect(result.current.session).toBeNull();
  });

  it('agent_session_transcript reject：error 态呈现（错误串透传）、不静默以空转录装载', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_sessions') {
        return Promise.resolve([sessionSummary({}, REPLAY_TURNS)]);
      }
      if (command === 'agent_session_transcript') {
        return Promise.reject('db: 转录读取失败');
      }
      return Promise.resolve(null);
    });

    const { result } = await mounted();

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('转录读取失败');
    expect(result.current.loading).toBe(false);
    expect(result.current.events).toEqual([]);
    // 重放原子应用：装载链任一环失败即整体不落地（会话镜像与 chain 均保持空态，
    // 不产半成品重建）
    expect(result.current.session).toBeNull();
    expect(result.current.chain).toEqual([]);
  });
});
