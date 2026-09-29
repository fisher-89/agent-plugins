import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../types/dto';
import { eventToChunk, type AgentUIMessageChunk } from './agent-adapter';
import { TauriAgentTransport } from './agent-transport';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_start 可切换 resolve/reject 并记录
// 入参）；Channel mock 为可编程 class（捕获 onmessage，测试直接投递
// AgentRunMessage 信封驱动流）。ReadableStream 与 chunk 消费用真实实现。
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
// fixture
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';

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

function recordRow(id: number, status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id,
    prompt: '调试一轮',
    cwd: ROOT,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000,
    finishedAt: status === 'running' ? null : 1727000001000,
    numTurns: null,
    costUsd: null,
    durationMs: null,
    sessionId: null,
    error: null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
  };
}

const CHAIN_BODY = {
  root: ROOT,
  prompt: '帮我跑一轮 loop',
  permissionMode: 'bypassPermissions' as const,
  resumeSessionId: 's-tail',
  parentRunId: 12,
  source: 'explore',
  sourceRef: '7',
};

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('sendMessages 未创建 Channel');
  return instance;
}

/** 从 transport 读流并收集全部 chunk（流 error 则 promise reject）。 */
async function drain(stream: ReadableStream<AgentUIMessageChunk>): Promise<AgentUIMessageChunk[]> {
  const reader = stream.getReader();
  const collected: AgentUIMessageChunk[] = [];
  for (;;) {
    const next = await reader.read();
    if (next.done) break;
    collected.push(next.value);
  }
  return collected;
}

function send(transport: TauriAgentTransport, body: object = CHAIN_BODY) {
  return transport.sendMessages({
    trigger: 'submit-message',
    chatId: 'chat-1',
    messageId: undefined,
    messages: [],
    abortSignal: undefined,
    body,
  });
}

function startCallArgs(): Record<string, unknown> {
  const call = invokeMock.mock.calls.filter(([name]) => name === 'agent_start').at(-1);
  if (!call) throw new Error('agent_start 未被调用');
  return call[1] as Record<string, unknown>;
}

beforeEach(() => {
  invokeMock.mockReset();
  ChannelMock.instances.length = 0;
});

// ---------------------------------------------------------------------------
// sendMessages：body 穿透与事件转 chunk
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：sendMessages', () => {
  it('body 链参数原样进入 agent_start 参数，transport 不增删改写任何字段', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    void stream.cancel();

    const args = startCallArgs();
    expect(args).toEqual({ ...CHAIN_BODY, onEvent: expect.any(ChannelMock) });
    expect(args['root']).toBe(CHAIN_BODY.root);
    expect(args['prompt']).toBe(CHAIN_BODY.prompt);
    expect(args['permissionMode']).toBe('bypassPermissions');
    expect(args['resumeSessionId']).toBe('s-tail');
    expect(args['parentRunId']).toBe(12);
    expect(args['source']).toBe('explore');
    expect(args['sourceRef']).toBe('7');
  });

  it('Channel 事件信封逐条经 eventToChunk 转 chunk 流出，顺序保序不重排', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();
    const events = [runStarted(0), textEvent(1, '正文')];

    const stream = await send(transport);
    const pumping = drain(stream);
    for (const event of events) {
      lastChannel().onmessage?.({ ipc: 'event', event });
    }
    const row = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    const recordChunks: AgentUIMessageChunk[] = [
      {
        type: 'start',
        messageId: 'run-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ];
    expect(collected).toEqual([...events.flatMap((event) => eventToChunk(event)), ...recordChunks]);
  });

  it('Record 信封 → data-run-record 部件 + finish 后可读流关闭，后续信封无 chunk', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();
    const row = recordRow(13, 'completed');

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected).toEqual([
      {
        type: 'start',
        messageId: 'run-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ]);
    // 流已关闭：后续信封（含事件）不再产出 chunk
    lastChannel().onmessage?.({ ipc: 'event', event: runStarted(0) });
    expect(collected).toHaveLength(4);
  });
});

// ---------------------------------------------------------------------------
// constructor：观测点透传
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：构造观测点', () => {
  it('注入 onEvent / onRecord 后逐条透传（含启动记录与终态 record）', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const onEvent = vi.fn();
    const onRecord = vi.fn();
    const transport = new TauriAgentTransport({ onEvent, onRecord });
    const row = recordRow(13, 'completed');

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'event', event: runStarted(0) });
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    await pumping;

    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(onEvent).toHaveBeenCalledWith(runStarted(0));
    // 启动 resolve 的 running 记录 + Channel 终态 record 双路透传
    expect(onRecord).toHaveBeenCalledTimes(2);
    expect(onRecord).toHaveBeenNthCalledWith(1, recordRow(13, 'running'));
    expect(onRecord).toHaveBeenNthCalledWith(2, row);
  });

  it('默认构造（无 options）不崩、不透传：信封投递与启动 resolve 均静默', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    expect(() => lastChannel().onmessage?.({ ipc: 'event', event: runStarted(0) })).not.toThrow();
    lastChannel().onmessage?.({ ipc: 'record', record: recordRow(13, 'completed') });
    const collected = await pumping;

    expect(collected.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// sendMessages：启动失败（异常）
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：启动失败', () => {
  it('agent_start reject → sendMessages 的流以错误 reject，不产出任何 chunk', async () => {
    invokeMock.mockRejectedValue(new Error('CLI 未找到'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const reader = stream.getReader();

    await expect(reader.read()).rejects.toThrow('CLI 未找到');
  });

  it('非 Error 形态的 reject 原因包一层 Error 再透出（错误串保留）', async () => {
    invokeMock.mockRejectedValue('store 打开失败');
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const reader = stream.getReader();

    await expect(reader.read()).rejects.toThrow('store 打开失败');
  });

  it('缺少链参数（body 无 root/prompt/source）→ 参数校验先行拒绝，不发起 invoke', async () => {
    const transport = new TauriAgentTransport();

    await expect(send(transport, {})).rejects.toThrow('缺少 agent_start 链参数');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('清单外 permissionMode → 参数校验拒绝并列出非法值', async () => {
    const transport = new TauriAgentTransport();

    await expect(send(transport, { ...CHAIN_BODY, permissionMode: 'yolo' })).rejects.toThrow(
      '非法 permissionMode',
    );
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// sendMessages：边界（空流即终 / 未知信封 / reconnect）
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：边界', () => {
  it('Channel 无 Event 直达 Record → 仅 record 部件 + finish 关流', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();
    const row = recordRow(13, 'stopped');

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected.map((chunk) => chunk.type)).toEqual([
      'start',
      'reset-step',
      'data-run-record',
      'finish',
    ]);
    expect(collected[2]?.type === 'data-run-record' && collected[2].data.status).toBe('stopped');
  });

  it('ipc 判别值既非 event 也非 record 的消息被忽略，流不断开', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'mystery', payload: '?' });
    lastChannel().onmessage?.({});
    lastChannel().onmessage?.({ ipc: 'record', record: recordRow(13, 'completed') });
    const collected = await pumping;

    // 未知信封不产出 chunk、不关流；record 照常收尾
    expect(collected.map((chunk) => chunk.type)).toEqual([
      'start',
      'reset-step',
      'data-run-record',
      'finish',
    ]);
  });

  it('reconnectToStream：任意 chatId 调用恒返回 null（重连由重放承担）', async () => {
    const transport = new TauriAgentTransport();

    const reconnected = await transport.reconnectToStream();
    expect(reconnected).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）与 dto shim 兼容（AC-6）：裸 invoke 切生成
// 绑定入口后，命令名 / 参数 key camelCase / 错误通道逐字不变（生成绑定底层
// 仍走 @tauri-apps/api/core 的 invoke，mock 机制切换后依旧生效）；手写镜像
// interface 退役后，fixture 类型导入改自生成物（经 dto shim）。
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：生成绑定调用面', () => {
  it('经生成绑定入口发起后 invoke 收到 "agent_start"，链参数 7 字段 camelCase key 与值逐字不变且 body 原样穿透', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    void stream.cancel();

    // 生成绑定位置参数 → invoke 参数对象：恰 8 个 key（链参数 7 + onEvent）
    expect(Object.keys(startCallArgs()).sort()).toEqual([
      'onEvent',
      'parentRunId',
      'permissionMode',
      'prompt',
      'resumeSessionId',
      'root',
      'source',
      'sourceRef',
    ]);
    expect(startCallArgs()).toEqual({ ...CHAIN_BODY, onEvent: expect.any(ChannelMock) });
  });

  it('Channel<AgentRunMessage> 信封（ipc event / record 双变体）类型来自生成物：事件转 chunk 保序、Record 收尾关流、未知信封忽略流不断', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'mystery', payload: '?' });
    lastChannel().onmessage?.({ ipc: 'event', event: runStarted(0) });
    lastChannel().onmessage?.({ ipc: 'record', record: recordRow(13, 'completed') });
    const collected = await pumping;

    // 未知信封不产出 chunk、流不断；事件 chunk 在前（保序），Record 四部件收尾关流
    const types = collected.map((chunk) => chunk.type);
    expect(types.length).toBeGreaterThan(4);
    expect(types.slice(-4)).toEqual(['start', 'reset-step', 'data-run-record', 'finish']);
  });

  it('生成绑定错误通道（Result<T, String> → reject）下启动失败：错误串透传、流以错误 reject、非 Error 原因包装行为不变', async () => {
    invokeMock.mockRejectedValue('CLI 未找到');
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const reader = stream.getReader();

    // 错误串经 reject 通道抵达，非 Error 原因包一层 Error 后透出
    await expect(reader.read()).rejects.toThrow('CLI 未找到');
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);
  });

  it("dto shim（from '../types/dto' 导入）在 shim 化后编译与运行不受影响：recordRow fixture 与断言照常工作", async () => {
    // AC-6 自动化半边：漏网旧 import 经纯 re-export shim 不炸——fixture 构造
    // （AgentRunRecord / AgentEvent 类型自 dto shim 导入）与断言语义不变
    const row: AgentRunRecord = recordRow(7, 'stopped');
    const event: AgentEvent = runStarted(0);
    expect([row.env, row.permissionMode, row.status]).toEqual([
      'default',
      'bypassPermissions',
      'stopped',
    ]);
    expect(event.kind).toBe('runStarted');

    invokeMock.mockResolvedValue(recordRow(7, 'running'));
    const transport = new TauriAgentTransport();
    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    const recordChunk = collected.find((chunk) => chunk.type === 'data-run-record');
    expect(recordChunk?.type === 'data-run-record' && recordChunk.data).toEqual(row);
  });
});
