import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, TurnSummary } from '../types/dto';
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

function textDelta(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

/** 子代理归因键的 delta（每配对键独立簿记断言的 fixture 半边）。 */
function keyedTextDelta(seq: number, parentToolUseId: string, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'messageDelta',
    parentToolUseId,
    delta: { kind: 'text', text },
  };
}

/** thinking 增量（推理流半边；跨轮再流式回归的 fixture）。 */
function thinkingDelta(seq: number, thinking: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'thinking', thinking },
  };
}

/** 密封 assistant 消息（携工具调用）：sdk loop 轮末收口事件形态。 */
function toolUseEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }],
    parentToolUseId: null,
  };
}

/** 密封 tool 结果消息（工具执行回灌的事件形态）。 */
function toolResultEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'message',
    role: 'tool',
    blocks: [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }],
    parentToolUseId: null,
  };
}

function recordRow(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: status === 'running' ? 's-live' : 's-done',
    status,
    startedAt: 1727000000000,
    finishedAt: status === 'running' ? null : 1727000001000,
    numTurns: null,
    costUsd: null,
    durationMs: null,
    error: null,
  };
}

const SESSION_BODY = {
  root: ROOT,
  prompt: '帮我跑一轮 loop',
  permissionMode: 'bypassPermissions' as const,
  sessionId: 'ses-0-1727000000000',
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

function send(transport: TauriAgentTransport, body: object = SESSION_BODY) {
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
  it('body 会话参数原样进入 agent_start 参数，transport 不增删改写任何字段', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    void stream.cancel();

    const args = startCallArgs();
    // agent 键恒在（body 无 agent → 缺席值 null 传递）
    expect(args).toEqual({ ...SESSION_BODY, agent: null, onEvent: expect.any(ChannelMock) });
    expect(args['root']).toBe(SESSION_BODY.root);
    expect(args['prompt']).toBe(SESSION_BODY.prompt);
    expect(args['permissionMode']).toBe('bypassPermissions');
    expect(args['sessionId']).toBe(SESSION_BODY.sessionId);
    expect(args['source']).toBe('explore');
    expect(args['sourceRef']).toBe('7');
    expect(args['agent']).toBeNull();
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
        messageId: 'turn-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ];
    expect(collected).toEqual([...events.flatMap((event) => eventToChunk(event)), ...recordChunks]);
  });

  it('流内首 delta 簿记：首个 messageDelta 补发 provisional 开件组（start + reset-step + part-start），后续 delta 直发累积 chunk', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(0, '你') });
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(1, '好') });
    // Record 信封收尾闭流（否则可读流不结束）
    const row = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected).toEqual([
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 0, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'text-start', id: 't-delta-' },
      { type: 'text-delta', id: 't-delta-', delta: '你' },
      { type: 'text-delta', id: 't-delta-', delta: '好' },
      {
        type: 'start',
        messageId: 'turn-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ]);
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
        messageId: 'turn-13',
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
  it('注入 onEvent / onRecord 后逐条透传（含启动轮行与终态轮行）', async () => {
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
    // 启动 resolve 的 running 轮行 + Channel 终态轮行双路透传
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

  it('缺少会话参数（body 无 root/prompt/source）→ 参数校验先行拒绝，不发起 invoke', async () => {
    const transport = new TauriAgentTransport();

    await expect(send(transport, {})).rejects.toThrow('缺少 agent_start 会话参数');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('清单外 permissionMode → 参数校验拒绝并列出非法值', async () => {
    const transport = new TauriAgentTransport();

    await expect(send(transport, { ...SESSION_BODY, permissionMode: 'yolo' })).rejects.toThrow(
      '非法 permissionMode',
    );
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// readSessionParams agent 读取：agent 可缺席（null → 后端解析默认 agent），
// 清单外值拒绝；sessionId 读取沿用 string | null 口径
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：agent 会话参数', () => {
  it('body.agent=5 / 9 → 读取透传至 invoke 位置参数', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));

    for (const agent of [5, 9]) {
      const transport = new TauriAgentTransport();
      const stream = await send(transport, { ...SESSION_BODY, agent });
      void stream.cancel();
      expect(startCallArgs()['agent']).toBe(agent);
    }
  });

  it('body 无 agent → agent=null 传递不抛错（缺席校验为 null，explore 链缺省安全）', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    // SESSION_BODY 不携 agent：readSessionParams 缺席承接 null，invoke 照常发起
    const stream = await send(transport);
    void stream.cancel();

    expect(startCallArgs()['agent']).toBeNull();
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_start')).toHaveLength(1);
  });

  it('body.agent 显式 null → agent=null 传递（与缺席同口径）', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport, { ...SESSION_BODY, agent: null });
    void stream.cancel();

    expect(startCallArgs()['agent']).toBeNull();
  });

  it('body.sessionId=null（New 语义）→ sessionId=null 传递不抛错', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport, { ...SESSION_BODY, sessionId: null });
    void stream.cancel();

    expect(startCallArgs()['sessionId']).toBeNull();
  });

  it("body.agent='yolo'（清单外值）→ 「非法 agent」参数校验拒绝且不发起 invoke（对齐 permissionMode 校验口径）", async () => {
    const transport = new TauriAgentTransport();

    await expect(send(transport, { ...SESSION_BODY, agent: 'yolo' })).rejects.toThrow('非法 agent');
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
// 仍走 @tauri-apps/api/core 的 invoke，mock 机制切换后依旧生效）；fixture
// 类型导入自生成物（经 dto shim）。
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：生成绑定调用面', () => {
  it('经生成绑定入口发起后 invoke 收到 "agent_start"，会话参数 7 字段 camelCase key 与值逐字不变且 body 原样穿透', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    void stream.cancel();

    // 生成绑定位置参数 → invoke 参数对象：恰 8 个 key（会话参数 7 + onEvent）
    expect(Object.keys(startCallArgs()).sort()).toEqual([
      'agent',
      'onEvent',
      'permissionMode',
      'prompt',
      'root',
      'sessionId',
      'source',
      'sourceRef',
    ]);
    expect(startCallArgs()).toEqual({
      ...SESSION_BODY,
      agent: null,
      onEvent: expect.any(ChannelMock),
    });
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
    //（TurnSummary / AgentEvent 类型自 dto shim 导入）与断言语义不变
    const row: TurnSummary = recordRow(7, 'stopped');
    const event: AgentEvent = runStarted(0);
    expect([row.sessionId, row.status]).toEqual(['s-done', 'stopped']);
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

// ---------------------------------------------------------------------------
// 补遗：每配对键独立簿记 / 无状态转换器约定（AC-1 前端半边锚点）
// ---------------------------------------------------------------------------

describe('TauriAgentTransport：流内簿记语义', () => {
  it('每配对键独立簿记：键 null 与 tu_1 并行时各键独立开件互不串件（part id 按键派生）', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    // 四枚 delta 交错投递：null 键两枚、tu_1 键两枚
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(0, '主通道首片') });
    lastChannel().onmessage?.({ ipc: 'event', event: keyedTextDelta(1, 'tu_1', '子代理甲') });
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(2, '主通道续片') });
    lastChannel().onmessage?.({ ipc: 'event', event: keyedTextDelta(3, 'tu_1', '子代理乙') });
    const row = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected).toEqual([
      // null 键首 delta：独立开件组（provisionalMessageId(null) 键）
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 0, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'text-start', id: 't-delta-' },
      { type: 'text-delta', id: 't-delta-', delta: '主通道首片' },
      // tu_1 键首 delta：独立开件组（provisionalMessageId('tu_1') 键）——
      // 不与 null 键串件（每配对键独立簿记半边）
      {
        type: 'start',
        messageId: 'delta-tu_1',
        messageMetadata: { seq: 1, parentToolUseId: 'tu_1' },
      },
      { type: 'reset-step' },
      { type: 'text-start', id: 't-delta-tu_1' },
      { type: 'text-delta', id: 't-delta-tu_1', delta: '子代理甲' },
      // 各键后续 delta 直发累积，part id 恒定归位各自通道
      { type: 'text-delta', id: 't-delta-', delta: '主通道续片' },
      { type: 'text-delta', id: 't-delta-tu_1', delta: '子代理乙' },
      {
        type: 'start',
        messageId: 'turn-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ]);
  });

  it('跨轮再流式回归：密封事件让位（reset-step 清活跃 part）后，下一批 delta 重新开件——reasoning-delta 前必有 reasoning-start', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    // 轮一：thinking 增量流式 → 密封 assistant（携工具调用）→ 工具结果密封
    lastChannel().onmessage?.({ ipc: 'event', event: thinkingDelta(0, '先想一步') });
    lastChannel().onmessage?.({ ipc: 'event', event: toolUseEvent(1) });
    lastChannel().onmessage?.({ ipc: 'event', event: toolResultEvent(2) });
    // 轮二：模型再次流式，thinking delta 再至（裸 delta 会在 reducer 抛
    // missing reasoning part "r-delta-"，即 agent debug 运行发起失败报错）
    lastChannel().onmessage?.({ ipc: 'event', event: thinkingDelta(3, '再想一步') });
    const row = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected).toEqual([
      // 轮一 thinking 首片：完整开件组
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 0, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'reasoning-start', id: 'r-delta-' },
      { type: 'reasoning-delta', id: 'r-delta-', delta: '先想一步' },
      // 密封 assistant（工具调用）让位替换
      {
        type: 'start',
        messageId: 'evt-1',
        messageMetadata: { seq: 1, parentToolUseId: null },
      },
      { type: 'reset-step' },
      {
        type: 'tool-input-available',
        toolCallId: 'tu_1',
        toolName: 'Bash',
        input: { command: 'ls' },
      },
      // 密封 tool 结果让位（无主 toolResult 就地合成占位部件）
      {
        type: 'start',
        messageId: 'evt-2',
        messageMetadata: { seq: 2, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'tool-input-available', toolCallId: 'tu_1', toolName: '工具调用', input: null },
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: '目录内容', isError: false },
      },
      // 轮二 thinking 再至：簿记已重武装，重新补发完整开件组
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 3, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'reasoning-start', id: 'r-delta-' },
      { type: 'reasoning-delta', id: 'r-delta-', delta: '再想一步' },
      // Record 收尾
      {
        type: 'start',
        messageId: 'turn-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ]);
  });

  it('同键跨 kind 补 part-start：text 增量后 thinking 增量到达仅补 reasoning-start（不再 reset-step 剪除已累积文本），同 kind 后续直发累积', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    const stream = await send(transport);
    const pumping = drain(stream);
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(0, '正文先到') });
    lastChannel().onmessage?.({ ipc: 'event', event: thinkingDelta(1, '思考后至') });
    lastChannel().onmessage?.({ ipc: 'event', event: thinkingDelta(2, '思考续片') });
    const row = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: row });
    const collected = await pumping;

    expect(collected).toEqual([
      // text 首片：完整开件组
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 0, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'text-start', id: 't-delta-' },
      { type: 'text-delta', id: 't-delta-', delta: '正文先到' },
      // 同键新 kind：仅补 part-start（无 start / reset-step，文本部件不被剪除）
      { type: 'reasoning-start', id: 'r-delta-' },
      { type: 'reasoning-delta', id: 'r-delta-', delta: '思考后至' },
      // 同 kind 后续：直发累积 chunk
      { type: 'reasoning-delta', id: 'r-delta-', delta: '思考续片' },
      // Record 收尾
      {
        type: 'start',
        messageId: 'turn-13',
        messageMetadata: { seq: null, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'data-run-record', data: row },
      { type: 'finish' },
    ]);
  });

  it('无状态转换器约定：同一 transport 连续两次 sendMessages，第二流首 delta 再次补发开件组（簿记为流闭包局部状态，跨流不残留）', async () => {
    invokeMock.mockResolvedValue(recordRow(13, 'running'));
    const transport = new TauriAgentTransport();

    // 流一：首 delta 簿记开件后以 Record 收尾
    const firstStream = await send(transport);
    const pumpingFirst = drain(firstStream);
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(0, '第一流增量') });
    const rowOne = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: rowOne });
    const first = await pumpingFirst;

    // 流二：同一 transport 再次 sendMessages（新 Channel 实例）
    const secondStream = await send(transport);
    const pumpingSecond = drain(secondStream);
    lastChannel().onmessage?.({ ipc: 'event', event: textDelta(7, '第二流增量') });
    const rowTwo = recordRow(13, 'completed');
    lastChannel().onmessage?.({ ipc: 'record', record: rowTwo });
    const second = await pumpingSecond;

    // 第二流首 delta 再次补发完整开件组（start + reset-step + part-start）——
    // 簿记状态为流闭包局部，第一流的已开件不残留到第二流
    expect(second.slice(0, 4)).toEqual([
      {
        type: 'start',
        messageId: 'delta-none',
        messageMetadata: { seq: 7, parentToolUseId: null },
      },
      { type: 'reset-step' },
      { type: 'text-start', id: 't-delta-' },
      { type: 'text-delta', id: 't-delta-', delta: '第二流增量' },
    ]);
    // 两流对同输入的转换行为一致（收尾 Record 组逐块相同，无跨流泄漏）
    expect(second.slice(-4)).toEqual(first.slice(-4));
    expect(second[second.length - 1]).toEqual({ type: 'finish' });
  });
});
