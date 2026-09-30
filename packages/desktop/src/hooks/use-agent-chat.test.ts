import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { eventsToUIMessages, runRecordToUIMessage } from '../lib/agent-adapter';
import type { AgentEvent, AgentRunRecord } from '../types/dto';
import { useAgentChat, type UseAgentChatParams } from './use-agent-chat';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_run_chain / agent_run_events /
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

const DEBUG_PARAMS: UseAgentChatParams = { source: 'explore', sourceRef: SOURCE_REF, root: ROOT };

function run(
  id: number,
  sessionId: string | null,
  parentRunId: number | null,
  status: AgentRunRecord['status'] = 'completed',
): AgentRunRecord {
  return {
    id,
    prompt: `explore 轮次 ${id}`,
    cwd: ROOT,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000 + id,
    finishedAt: status === 'running' ? null : 1727000001000 + id,
    numTurns: status === 'running' ? null : 1,
    costUsd: status === 'running' ? null : 0.1,
    durationMs: status === 'running' ? null : 500,
    sessionId: status === 'running' ? null : sessionId,
    error: null,
    source: 'explore',
    sourceRef: SOURCE_REF,
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

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
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

/** 链 fixture：两条 parent_run_id 相连的 completed run（链头在前）。 */
const CHAIN = [run(11, 's-first', null), run(12, 's-tail', 11)];

const SEND_INPUT = {
  prompt: '继续追这条线索',
  permissionMode: 'bypassPermissions' as const,
};

// ---------------------------------------------------------------------------
// IPC 编排
// ---------------------------------------------------------------------------

let chainResult: AgentRunRecord[];
let eventsByRun: Map<number, AgentEvent[]>;
let startBehavior: { mode: 'resolve' | 'reject'; value: AgentRunRecord | string } | null = null;
let startCalls: number;

function mockIpc() {
  ChannelMock.instances.length = 0;
  chainResult = CHAIN.map((item) => ({ ...item }));
  eventsByRun = new Map<number, AgentEvent[]>([
    [11, [textEvent(0, '首轮结论')]],
    [12, [textEvent(0, '续轮结论'), runResultEvent(1)]],
  ]);
  startBehavior = null;
  startCalls = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'agent_run_chain') {
      return Promise.resolve(chainResult);
    }
    if (command === 'agent_run_events') {
      return Promise.resolve(eventsByRun.get(Number(params?.runId)) ?? []);
    }
    if (command === 'agent_start') {
      startCalls += 1;
      const behavior = startBehavior;
      if (behavior === null || behavior.mode === 'resolve') {
        return Promise.resolve(behavior === null ? run(13, null, null, 'running') : behavior.value);
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

function deliverRecord(row: AgentRunRecord) {
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
// 重放装载（链还原 + 逐 run 事件重放）
// ---------------------------------------------------------------------------

describe('useAgentChat：重放装载', () => {
  it('挂载发起 agent_run_chain 携 (root, source, sourceRef) 三参 + 逐 run agent_run_events 携 (root, run.id)，经适配层重建消息；链镜像与查询一致', async () => {
    const { result } = await mounted();

    const chainCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_run_chain');
    expect(chainCalls).toEqual([
      ['agent_run_chain', { root: ROOT, source: 'explore', sourceRef: SOURCE_REF }],
    ]);
    expect(result.current.chain).toEqual(CHAIN);
    expect(result.current.events).toEqual([
      textEvent(0, '首轮结论'),
      textEvent(0, '续轮结论'),
      runResultEvent(1),
    ]);
    expect(result.current.currentRunId).toBe(12);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    // 重建形状：逐 run 折叠 + 终态 record 部件交错（running 才省略 record）
    expect(result.current.messages).toEqual([
      ...eventsToUIMessages(eventsByRun.get(11) ?? []),
      runRecordToUIMessage(CHAIN[0]),
      ...eventsToUIMessages(eventsByRun.get(12) ?? []),
      runRecordToUIMessage(CHAIN[1]),
    ]);
  });

  it('重放装载重建的 messages 与同批事件经 transport 实时流入 useChat 的终态形状一致（两路同构）', async () => {
    chainResult = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));
    // 启动 resolve 的 running 记录确立 currentRunId，事件入桶
    act(() => {
      startBehavior = { mode: 'resolve', value: run(13, 's-new', 12, 'running') };
    });
    await act(async () => {});
    const realtimeEvents = [runStarted(0), textEvent(1, '正文'), runResultEvent(2)];
    for (const event of realtimeEvents) {
      deliverEvent(event);
    }
    const terminal = run(13, 's-new', 12, 'completed');
    deliverRecord(terminal);

    await waitFor(() => expect(result.current.running).toBe(false));
    // 收口归一后终态 = 折叠重建形状（重开恢复 = 重放的同构锚点）
    expect(result.current.messages).toEqual([
      ...eventsToUIMessages(realtimeEvents),
      runRecordToUIMessage(terminal),
    ]);
    expect(result.current.events).toEqual(realtimeEvents);
    expect(result.current.chain).toEqual([terminal]);
    expect(result.current.currentRunId).toBe(13);
  });
});

// ---------------------------------------------------------------------------
// 发送组装 / stop 接线 / 运行门闩
// ---------------------------------------------------------------------------

describe('useAgentChat：发送组装与停止', () => {
  it('sendMessage 经 body 携带链尾非 running 记录的 resumeSessionId / parentRunId，三元组与 root 透传 agent_start', async () => {
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
      resumeSessionId: 's-tail',
      parentRunId: 12,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });
    expect(args['onEvent']).toBeInstanceOf(ChannelMock);
  });

  it('stop() 调 invoke("agent_stop", { root, runId }) 复合键寻址且不截断前端流：Record 回流后 running 复位、消息含 record 部件', async () => {
    chainResult = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));
    act(() => {
      startBehavior = { mode: 'resolve', value: run(13, null, null, 'running') };
    });
    await act(async () => {});
    deliverEvent(textEvent(0, '停止前已产出'));

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledWith('agent_stop', { root: ROOT, runId: 13 });

    // 前端流不被截断：stop 后终态 record 经 Channel 回流并复位 running
    const terminal = run(13, null, null, 'stopped');
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

  it('状态 submitted/streaming 期间重复 sendMessage 被忽略，不并发第二条 run', async () => {
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

    // 收尾解锁：终态 record 回流后恢复可发
    act(() => {
      startBehavior = { mode: 'resolve', value: run(13, null, null, 'running') };
    });
    await act(async () => {});
    deliverRecord(run(13, null, null, 'completed'));
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
    expect(result.current.chain).toEqual(CHAIN);
  });
});

// ---------------------------------------------------------------------------
// 边界：链尾 running 过滤 / 空链 / null 守卫 / reset
// ---------------------------------------------------------------------------

describe('useAgentChat：边界', () => {
  it('链尾记录为 running（stop 后终态 record 未达）→ 发送取上一条非 running 记录的链尾参数', async () => {
    chainResult = [run(11, 's-first', null), run(12, null, 11, 'running')];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args['resumeSessionId']).toBe('s-first');
    expect(args['parentRunId']).toBe(11);
  });

  it('空链时 resume / parent 参数不携带（null），来源三元组仍齐全', async () => {
    chainResult = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});

    const args = startCallArgs();
    expect(args['resumeSessionId']).toBeNull();
    expect(args['parentRunId']).toBeNull();
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

  it('sourceRef 为 null（debug 场景）：不重放装载，发送以 null 穿透 source / 链参数，不崩', async () => {
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
    expect(args['resumeSessionId']).toBeNull();
    expect(args['parentRunId']).toBeNull();
  });

  it('reset() 清空 messages / events / chain / currentRunId（debug 页每跑重置场景）', async () => {
    const { result } = await mounted();
    expect(result.current.chain).toHaveLength(2);

    act(() => {
      result.current.reset();
    });

    expect(result.current.messages).toEqual([]);
    expect(result.current.events).toEqual([]);
    expect(result.current.chain).toEqual([]);
    expect(result.current.currentRunId).toBeNull();
    // 重置后可正常发起新一轮：空链语义（resume / parent 缺省 null）
    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await act(async () => {});
    const args = startCallArgs();
    expect(args['resumeSessionId']).toBeNull();
    expect(args['parentRunId']).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：agent_run_chain / agent_run_events /
// agent_stop 裸 invoke → typed bindings 机械替换后，invoke 命令名与 camelCase
// 参数逐字不变（生成绑定底层仍走 @tauri-apps/api/core 的 invoke，mock 机制
// 切换后依旧生效）；root 寻址参数随双库拆分（desktop-workspace-db-split）
// 进入链还原与停止三条命令。
// ---------------------------------------------------------------------------

describe('useAgentChat：生成绑定调用面', () => {
  it('发起经生成绑定入口后 invoke 收到 "agent_start" 与 7 个 camelCase 链参数逐字不变，返回 running 记录透传', async () => {
    const { result } = await mounted();

    // 链发起：命令名与 root 寻址三元组逐字不变
    expect(invokeMock).toHaveBeenCalledWith('agent_run_chain', {
      root: ROOT,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    // 生成绑定位置参数 → invoke 参数对象：恰 8 个 key（链参数 7 + onEvent）
    const args = startCallArgs();
    expect(Object.keys(args).sort()).toEqual([
      'onEvent',
      'parentRunId',
      'permissionMode',
      'prompt',
      'resumeSessionId',
      'root',
      'source',
      'sourceRef',
    ]);
    expect(args).toMatchObject({
      root: ROOT,
      prompt: SEND_INPUT.prompt,
      permissionMode: 'bypassPermissions',
      resumeSessionId: 's-tail',
      parentRunId: 12,
      source: 'explore',
      sourceRef: SOURCE_REF,
    });
    // 返回 running 记录透传（提前 resolve：running 置位、id 可寻址）
    await waitFor(() => expect(result.current.currentRunId).toBe(13));
    expect(result.current.running).toBe(true);
  });

  it('逐 run 重放经生成绑定后 invoke 收到 "agent_run_events" 与 { root, runId }：空链零调用、多 run 逐 run 调用与消息重建不变', async () => {
    const { result } = await mounted();

    // 多 run：每 run 恰一次重放调用，参数逐字 { root, runId }，消息重建不变
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_run_events')).toEqual([
      ['agent_run_events', { root: ROOT, runId: 11 }],
      ['agent_run_events', { root: ROOT, runId: 12 }],
    ]);
    // 消息重建不变：逐 run 事件折叠 + 终态 record 部件交错（1+1+2+1 = 5 条）
    expect(result.current.messages).toHaveLength(5);

    // 空链：零 agent_run_events 调用（空链语义在绑定切换后保持）
    const callsBeforeEmpty = invokeMock.mock.calls.length;
    chainResult = [];
    const empty = await mounted();
    expect(
      invokeMock.mock.calls.slice(callsBeforeEmpty).filter(([name]) => name === 'agent_run_events'),
    ).toHaveLength(0);
    expect(empty.result.current.messages).toEqual([]);
  });

  it('停止经生成绑定后 invoke 收到 "agent_stop" 与 { root, runId }；reject 路径 error 置位 / running 复位行为不变', async () => {
    chainResult = [];
    const { result } = await mounted();

    act(() => {
      result.current.sendMessage(SEND_INPUT);
    });
    await waitFor(() => expect(result.current.running).toBe(true));

    act(() => {
      result.current.stop();
    });
    await act(async () => {});
    expect(invokeMock).toHaveBeenCalledWith('agent_stop', { root: ROOT, runId: 13 });

    // 终态 record 回流后 running 复位（停止路径行为不变）
    deliverRecord(run(13, null, null, 'stopped'));
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
