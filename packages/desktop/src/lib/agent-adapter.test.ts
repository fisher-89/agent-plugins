import { useChat } from '@ai-sdk/react';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ChatTransport } from 'ai';
import { describe, expect, it } from 'vite-plus/test';

import type { AgentBlock, AgentEvent, TurnSummary } from '../types/dto';
import * as adapterModule from './agent-adapter';
import {
  deltaPartId,
  eventToChunk,
  eventsToUIMessages,
  provisionalMessageId,
  runRecordToUIMessage,
  type AgentToolPart,
  type AgentUIMessage,
  type AgentUIMessageChunk,
} from './agent-adapter';

// ---------------------------------------------------------------------------
// 纯函数模块，无外部依赖（`ai` 仅类型引用），不需要 Mock。
// fixture 对齐 serde camelCase 线格式（types/dto 镜像）。
// ---------------------------------------------------------------------------

const TS = 1727000000000;

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash', 'Read'],
    mcpServers: ['mcp-a'],
  };
}

function message(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks,
    parentToolUseId,
  };
}

function userMessage(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function systemNotice(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'systemNotice',
    subtype: 'permission_denial',
    payload: { tool: 'Bash' },
  };
}

function turnDone(
  seq: number,
  fields: Partial<Extract<AgentEvent, { kind: 'turnDone' }>> = {},
): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.42,
    usage: { input_tokens: 10 },
    sessionId: 's-1',
    ...fields,
  };
}

function messageDelta(
  seq: number,
  delta: { kind: 'text'; text: string } | { kind: 'thinking'; thinking: string },
  parentToolUseId: string | null = null,
): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'messageDelta',
    parentToolUseId,
    delta,
  };
}

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery","note":"保留原文"}',
  };
}

function record(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: status === 'running' ? 's-live' : 's-done',
    status,
    startedAt: TS,
    finishedAt: status === 'running' ? null : TS + 999,
    numTurns: status === 'running' ? null : 2,
    costUsd: status === 'running' ? null : 0.2,
    durationMs: status === 'running' ? null : 800,
    error: status === 'failed' ? '进程结束但未产出收敛事件' : null,
  };
}

// ---------------------------------------------------------------------------
// eventsToUIMessages：五变体映射
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：五变体映射', () => {
  it('runStarted 产 data-run-started 部件：model / sessionId / tools / mcpServers 载荷保真', () => {
    const messages = eventsToUIMessages([runStarted(0)]);

    expect(messages).toHaveLength(1);
    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-run-started');
    expect(part?.type === 'data-run-started' && part.data).toEqual({
      seq: 0,
      timestampMs: TS,
      model: 'claude-opus',
      sessionId: 's-1',
      tools: ['Bash', 'Read'],
      mcpServers: ['mcp-a'],
    });
  });

  it('message.text 产 text 部件、message.thinking 产 reasoning 部件（整块无 token 拆分）', () => {
    const messages = eventsToUIMessages([
      message(1, [
        { kind: 'thinking', thinking: '先想一想' },
        { kind: 'text', text: '正文内容' },
      ]),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0]?.parts).toEqual([
      { type: 'reasoning', text: '先想一想', state: 'done' },
      { type: 'text', text: '正文内容', state: 'done' },
    ]);
  });

  it('message.toolUse 产 tool 部件：type=tool-<name> 且 toolCallId=block.id、input 保真', () => {
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
    ]);

    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-Bash',
        toolCallId: 'tu_1',
        state: 'input-available',
        input: { command: 'ls' },
      },
    ]);
  });

  it('systemNotice 产 data-system-notice 部件：subtype 与 payload 载荷保真', () => {
    const messages = eventsToUIMessages([systemNotice(2)]);

    expect(messages[0]?.parts[0]?.type).toBe('data-system-notice');
    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-system-notice' && part.data).toEqual({
      seq: 2,
      timestampMs: TS,
      subtype: 'permission_denial',
      payload: { tool: 'Bash' },
    });
  });

  it('turnDone 产 data-run-result 部件：subtype / usage 等汇总载荷保真', () => {
    const messages = eventsToUIMessages([turnDone(3)]);

    expect(messages[0]?.parts[0]?.type).toBe('data-run-result');
    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-run-result' && part.data).toEqual({
      seq: 3,
      timestampMs: TS,
      subtype: 'success',
      isError: false,
      numTurns: 3,
      durationMs: 1234,
      costUsd: 0.42,
      usage: { input_tokens: 10 },
      sessionId: 's-1',
    });
  });

  it('raw 产 data-raw 部件：eventType 与 rawJson 原文保真', () => {
    const messages = eventsToUIMessages([raw(4)]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-raw' && part.data).toEqual({
      seq: 4,
      timestampMs: TS,
      eventType: 'mystery',
      rawJson: '{"type":"mystery","note":"保留原文"}',
    });
  });
});

// ---------------------------------------------------------------------------
// eventsToUIMessages：消息形状约定（id / metadata / 角色收窄 / seq 保序）
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：消息形状约定', () => {
  it('message 事件每条产出一个 UIMessage：id=evt-<seq>、role 收窄 user/assistant、metadata 携带 seq 与 parentToolUseId', () => {
    const messages = eventsToUIMessages([
      userMessage(0, '帮我查'),
      message(1, [{ kind: 'text', text: '好的' }], 'tu_9'),
    ]);

    expect(messages.map((m) => m.id)).toEqual(['evt-0', 'evt-1']);
    expect(messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(messages[0]?.metadata).toEqual({ seq: 0, parentToolUseId: null });
    expect(messages[1]?.metadata).toEqual({ seq: 1, parentToolUseId: 'tu_9' });
  });

  it('tool role（工具结果管道）映射为 system：非对话行桶，对话透镜天然排除', () => {
    const toolResult = {
      ...message(5, [{ kind: 'toolResult', id: 'tu_1', content: 'ok', isError: false }]),
      role: 'tool',
    } as AgentEvent;
    const messages = eventsToUIMessages([
      message(4, [{ kind: 'toolUse', id: 'tu_1', name: 'read', input: {} }]),
      toolResult,
    ]);

    expect(messages.map((m) => m.role)).toEqual(['assistant', 'system']);
    // 已配对结果折叠进工具卡，tool 消息自身零部件（由呈现层不变式跳过空行）
    expect(messages[1]?.parts).toEqual([]);
  });

  it('非 message 事件各产出一条 system 角色消息携带单个 data 部件，seq 保序', () => {
    const events = [runStarted(0), systemNotice(1), turnDone(2), raw(3)];
    const messages = eventsToUIMessages(events);

    expect(messages.map((m) => m.id)).toEqual(['evt-0', 'evt-1', 'evt-2', 'evt-3']);
    expect(messages.map((m) => m.role)).toEqual(['system', 'system', 'system', 'system']);
    expect(messages.map((m) => m.parts.map((part) => part.type))).toEqual([
      ['data-run-started'],
      ['data-system-notice'],
      ['data-run-result'],
      ['data-raw'],
    ]);
  });
});

// ---------------------------------------------------------------------------
// eventsToUIMessages：工具同 id 配对
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：工具同 id 配对', () => {
  it('toolUse 块与跨消息同 id toolResult 合并为单 tool 部件（input + output-available 形态）', () => {
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
      message(2, [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }]),
    ]);

    expect(messages).toHaveLength(2);
    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-Bash',
        toolCallId: 'tu_1',
        state: 'output-available',
        input: { command: 'ls' },
        output: { content: '目录内容', isError: false },
      },
    ]);
    // 有主 result 不重复呈现：第二条消息不携带部件
    expect(messages[1]?.parts).toEqual([]);
  });

  it('isError 的 result 数据随配对并入 output', () => {
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: {} }]),
      message(2, [{ kind: 'toolResult', id: 'tu_1', content: 'boom', isError: true }]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'tool-Bash' && part.output).toEqual({
      content: 'boom',
      isError: true,
    });
  });

  it('重复 id 的 result 取首个，不重复续 output', () => {
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: {} }]),
      message(2, [{ kind: 'toolResult', id: 'tu_1', content: '首个结果', isError: false }]),
      message(3, [{ kind: 'toolResult', id: 'tu_1', content: '迟到结果', isError: false }]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'tool-Bash' && part.output).toEqual({
      content: '首个结果',
      isError: false,
    });
    // 两条 result 均已被 toolUse 侧收编：后续消息零部件
    expect(messages[1]?.parts).toEqual([]);
    expect(messages[2]?.parts).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// eventsToUIMessages：无主 toolResult（边界）
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：无主 toolResult', () => {
  it('无配对 toolUse 的 toolResult 就地合成占位部件（name=工具调用），result 数据不丢', () => {
    const messages = eventsToUIMessages([
      message(0, [{ kind: 'toolResult', id: 'orphan_1', content: '无主结果', isError: true }]),
    ]);

    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-工具调用',
        toolCallId: 'orphan_1',
        state: 'output-available',
        input: null,
        output: { content: '无主结果', isError: true },
      },
    ]);
  });
});

// ---------------------------------------------------------------------------
// eventsToUIMessages：空输入（边界）
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：空输入', () => {
  it('空事件数组 → 空消息序列', () => {
    expect(eventsToUIMessages([])).toEqual([]);
  });

  it('blocks 为空数组的 message 事件仍产出无部件的 UIMessage（消息不丢）', () => {
    const messages = eventsToUIMessages([message(0, [])]);

    expect(messages).toHaveLength(1);
    expect(messages[0]?.id).toBe('evt-0');
    expect(messages[0]?.parts).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// eventsToUIMessages：未知事件透传（边界）
// ---------------------------------------------------------------------------

describe('eventsToUIMessages：未知事件透传', () => {
  it('raw 变体（未知 event_type）→ data-raw 部件原文透传，原文 JSON 不被解释改写', () => {
    const original = '{"type":"mystery","nested":{"n":1},"note":"第一行\\n中文 🎉"}';
    const event: AgentEvent = {
      seq: 7,
      timestampMs: TS,
      kind: 'raw',
      eventType: 'totally-unknown',
      rawJson: original,
    };
    const messages = eventsToUIMessages([event]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-raw' && part.data.rawJson).toBe(original);
  });

  it('运行时未知事件变体（未来 CLI 新增）→ data-raw 整事件 JSON 透传不丢', () => {
    const timeTravel = {
      seq: 9,
      timestampMs: TS,
      kind: 'timeTravel',
      targetEpoch: 2,
    } as unknown as AgentEvent;
    const messages = eventsToUIMessages([timeTravel]);

    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-raw');
    expect(part?.type === 'data-raw' && part.data.rawJson).toBe(JSON.stringify(timeTravel));
  });
});

// ---------------------------------------------------------------------------
// runRecordToUIMessage：终态同构与字段保真
// ---------------------------------------------------------------------------

describe('runRecordToUIMessage', () => {
  it('任意 record → 单条消息携带 data-run-record 部件（data 即 record 整行），metadata.seq 为 null', () => {
    const row = record(11, 'completed');
    const messages = [runRecordToUIMessage(row)];

    expect(messages).toHaveLength(1);
    expect(messages[0]?.id).toBe('turn-11');
    expect(messages[0]?.role).toBe('system');
    expect(messages[0]?.metadata).toEqual({ seq: null, parentToolUseId: null });
    expect(messages[0]?.parts).toEqual([{ type: 'data-run-record', data: row }]);
  });

  it('completed / failed / stopped 三态的 status、error、finishedAt、sessionId 均可在 record 部件 data 中考证', () => {
    const completed = runRecordToUIMessage(record(1, 'completed'));
    const failed = runRecordToUIMessage({
      ...record(2, 'failed'),
      error: '进程结束但未产出 result 事件',
    });
    const stopped = runRecordToUIMessage(record(3, 'stopped'));

    const statuses = [completed, failed, stopped].map(
      (m) => m.parts[0]?.type === 'data-run-record' && m.parts[0].data.status,
    );
    expect(statuses).toEqual(['completed', 'failed', 'stopped']);
    const failedData = failed.parts[0];
    expect(failedData?.type === 'data-run-record' && failedData.data.error).toBe(
      '进程结束但未产出 result 事件',
    );
    const stoppedData = stopped.parts[0];
    expect(stoppedData?.type === 'data-run-record' && stoppedData.data.error).toBeNull();
    const completedData = completed.parts[0];
    expect(completedData?.type === 'data-run-record' && completedData.data.finishedAt).toBe(
      TS + 999,
    );
    expect(completedData?.type === 'data-run-record' && completedData.data.sessionId).toBe(
      's-done',
    );
  });
});

// ---------------------------------------------------------------------------
// 双层词汇：delta 防御跳过（密封-only 重放）+ provisional 键 / delta 路 chunk
// ---------------------------------------------------------------------------

describe('双层词汇：delta 防御跳过与 delta 路 chunk', () => {
  it('eventsToUIMessages 防御跳过 messageDelta（delta 仅实时流可见，不进重放路径）', () => {
    const messages = eventsToUIMessages([
      messageDelta(0, { kind: 'text', text: '增量' }),
      message(1, [{ kind: 'text', text: '密封' }]),
    ]);

    expect(messages.map((m) => m.id)).toEqual(['evt-1']);
  });

  it('eventToChunk 对 messageDelta 产出单条累积 delta chunk（text / thinking 可辨，稳定 part id 派生自配对键）', () => {
    const textChunks = eventToChunk(messageDelta(2, { kind: 'text', text: '你' }));
    const thinkingChunks = eventToChunk(
      messageDelta(3, { kind: 'thinking', thinking: '想' }, 'tu_1'),
    );

    expect(textChunks).toEqual([{ type: 'text-delta', id: 't-delta-', delta: '你' }]);
    expect(thinkingChunks).toEqual([{ type: 'reasoning-delta', id: 'r-delta-tu_1', delta: '想' }]);
  });

  it('provisionalMessageId 配对键派生（None 单通道）', () => {
    expect(provisionalMessageId(null)).toBe('delta-none');
    expect(provisionalMessageId('tu_1')).toBe('delta-tu_1');
  });
});

// ---------------------------------------------------------------------------
// eventToChunk：两路同构不变量（真实 useChat reducer 归约）
// ---------------------------------------------------------------------------

/** 同构投影：实时路 reducer 消息角色统一由 hook 收口归一承载，投影取 id / metadata / parts */
function shapeOf(messages: AgentUIMessage[]): unknown[] {
  return messages.map((m) => ({
    id: m.id,
    metadata: m.metadata,
    parts: m.parts.map((part) => {
      if (part.type === 'text' || part.type === 'reasoning') {
        return { type: part.type, text: part.text, state: part.state };
      }
      if (part.type.startsWith('tool-')) {
        const tool = part as AgentToolPart;
        return {
          type: tool.type,
          toolCallId: tool.toolCallId,
          state: tool.state,
          input: tool.input,
          output: tool.state === 'output-available' ? tool.output : undefined,
        };
      }
      return { type: part.type, data: (part as { data: unknown }).data };
    }),
  }));
}

/** 空传输：把预生成的 chunk 序列一次性回放（reducer 归约驱动，不经进程边界） */
function replayTransport(chunks: AgentUIMessageChunk[]): ChatTransport<AgentUIMessage> {
  return {
    sendMessages: async () =>
      new ReadableStream<AgentUIMessageChunk>({
        start(controller) {
          for (const chunk of chunks) controller.enqueue(chunk);
          controller.close();
        },
      }),
    reconnectToStream: async () => null,
  };
}

/** 同批事件经 eventToChunk 逐条流入 useChat reducer 归约后的 evt-* 消息序列 */
async function reducedMessages(events: AgentEvent[]): Promise<AgentUIMessage[]> {
  const chunks: AgentUIMessageChunk[] = [
    ...events.flatMap((event) => eventToChunk(event)),
    { type: 'finish' },
  ];
  const rendered = renderHook(() =>
    useChat<AgentUIMessage>({ transport: replayTransport(chunks) }),
  );
  await act(async () => {
    await rendered.result.current.sendMessage({ text: '驱动发送' });
  });
  await waitFor(() => expect(rendered.result.current.status).toBe('ready'));
  return rendered.result.current.messages.filter((m) => m.id.startsWith('evt-'));
}

describe('eventToChunk：两路同构不变量', () => {
  it('同一事件序列经 eventsToUIMessages 折叠与经 eventToChunk 流入 useChat reducer 归约，终态形状一致', async () => {
    const events = [
      runStarted(0),
      userMessage(1, '帮我查'),
      message(2, [
        { kind: 'thinking', thinking: '想一想' },
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
        { kind: 'text', text: '结论' },
      ]),
      systemNotice(3),
      raw(4),
      turnDone(5),
    ];

    const reduced = await reducedMessages(events);

    expect(reduced.map((m) => m.id)).toEqual([
      'evt-0',
      'evt-1',
      'evt-2',
      'evt-3',
      'evt-4',
      'evt-5',
    ]);
    expect(shapeOf(reduced)).toEqual(shapeOf(eventsToUIMessages(events)));
  });
});

// ---------------------------------------------------------------------------
// eventToChunk：增量边界
// ---------------------------------------------------------------------------

describe('eventToChunk：增量边界', () => {
  it('每组以 start（messageId=evt-<seq>，metadata 保真）+ reset-step 开头', () => {
    const chunks = eventToChunk(userMessage(3, '继续'));

    expect(chunks[0]).toEqual({
      type: 'start',
      messageId: 'evt-3',
      messageMetadata: { seq: 3, parentToolUseId: null },
    });
    expect(chunks[1]).toEqual({ type: 'reset-step' });
  });

  it('实时到达的无主 toolResult 产出就地合成占位部件的 chunk 组，与折叠路径语义一致', () => {
    const chunks = eventToChunk(
      message(0, [{ kind: 'toolResult', id: 'orphan_1', content: '无主结果', isError: false }]),
    );

    expect(chunks.slice(2)).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'orphan_1',
        toolName: '工具调用',
        input: null,
      },
      {
        type: 'tool-output-available',
        toolCallId: 'orphan_1',
        output: { content: '无主结果', isError: false },
      },
    ]);
  });

  it('同事件内 toolUse 与 result 配对就地续 output（tool-input-available + tool-output-available）', () => {
    const chunks = eventToChunk(
      message(0, [
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        { kind: 'toolResult', id: 'tu_1', content: 'ok', isError: false },
      ]),
    );

    expect(chunks.slice(2)).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'tu_1',
        toolName: 'Bash',
        input: { command: 'ls' },
      },
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: 'ok', isError: false },
      },
    ]);
  });

  it('空 blocks 事件产出不含部件的 chunk 组（仅 start + reset-step），不抛错', () => {
    const chunks = eventToChunk(message(0, []));

    expect(chunks).toEqual([
      { type: 'start', messageId: 'evt-0', messageMetadata: { seq: 0, parentToolUseId: null } },
      { type: 'reset-step' },
    ]);
  });

  it('raw 事件产出 data-raw chunk，不抛错', () => {
    const chunks = eventToChunk(raw(1));

    expect(chunks.slice(2)).toEqual([
      {
        type: 'data-raw',
        data: {
          seq: 1,
          timestampMs: TS,
          eventType: 'mystery',
          rawJson: '{"type":"mystery","note":"保留原文"}',
        },
      },
    ]);
  });
});

// ---------------------------------------------------------------------------
// eventToChunk：实时路跨消息配对视野（currentMessageToolCalls）
// ---------------------------------------------------------------------------

describe('eventToChunk：实时路跨消息配对视野', () => {
  const resultEvent = (seq: number, id: string, content: string): AgentEvent =>
    message(seq, [{ kind: 'toolResult', id, content, isError: false }]);

  it('命中视野的纯结果事件只投 tool-output-available：无消息头、不重投 input（与 toolUse 合住一条消息）', () => {
    const chunks = eventToChunk(resultEvent(2, 'tu_1', '目录内容'), new Set(['tu_1']));

    expect(chunks).toEqual([
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: '目录内容', isError: false },
      },
    ]);
  });

  it('未命中视野（id 不在集内 / 空集缺省）→ 既有占位卡 chunk 组（缺省参零漂移）', () => {
    for (const window of [new Set(['tu_other']), new Set<string>()]) {
      const chunks = eventToChunk(resultEvent(2, 'tu_1', '目录内容'), window);
      expect(chunks.map((chunk) => chunk.type)).toEqual([
        'start',
        'reset-step',
        'tool-input-available',
        'tool-output-available',
      ]);
    }
    expect(eventToChunk(resultEvent(2, 'tu_1', '目录内容'))).toEqual(
      eventToChunk(resultEvent(2, 'tu_1', '目录内容'), new Set()),
    );
  });

  it('同事件混含命中与未命中结果：命中项先就地补全（先于消息头），其余块另起新消息', () => {
    const chunks = eventToChunk(
      message(2, [
        { kind: 'toolResult', id: 'tu_1', content: '命中结果', isError: false },
        { kind: 'toolResult', id: 'tu_9', content: '未命中结果', isError: true },
      ]),
      new Set(['tu_1']),
    );

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      'tool-output-available',
      'start',
      'reset-step',
      'tool-input-available',
      'tool-output-available',
    ]);
    expect(chunks[0]).toEqual({
      type: 'tool-output-available',
      toolCallId: 'tu_1',
      output: { content: '命中结果', isError: false },
    });
    expect(chunks[3]).toEqual({
      type: 'tool-input-available',
      toolCallId: 'tu_9',
      toolName: '工具调用',
      input: null,
    });
  });

  it('重复 id 结果只补首个、后继块不再呈现（与折叠路同口径）', () => {
    const chunks = eventToChunk(
      message(2, [
        { kind: 'toolResult', id: 'tu_1', content: '首个结果', isError: false },
        { kind: 'toolResult', id: 'tu_1', content: '迟到结果', isError: false },
      ]),
      new Set(['tu_1']),
    );

    expect(chunks).toEqual([
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: '首个结果', isError: false },
      },
    ]);
  });

  it('同事件另有同 id toolUse → 让位本事件 local 配对（不走跨消息补全）', () => {
    const chunks = eventToChunk(
      message(2, [
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'pwd' } },
        { kind: 'toolResult', id: 'tu_1', content: '内联结果', isError: false },
      ]),
      new Set(['tu_1']),
    );

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      'start',
      'reset-step',
      'tool-input-available',
      'tool-output-available',
    ]);
  });
});

// ---------------------------------------------------------------------------
// 补遗：deltaPartId 直调 / provisionalMessageId 边界 / 纯 delta 折叠为空 /
// runRecordToUIMessage running 形态（AC-8 锚点面）
// ---------------------------------------------------------------------------

describe('deltaPartId 直调：稳定性与互异契约', () => {
  it('同键同类重复调用幂等；text / thinking 两类互异；不同键互异', () => {
    // 同键同类幂等（delta 累积部件的稳定 part id 契约）
    expect(deltaPartId('tu_1', 'text')).toBe(deltaPartId('tu_1', 'text'));
    expect(deltaPartId('tu_1', 'thinking')).toBe(deltaPartId('tu_1', 'thinking'));
    // 线格式逐字（与 eventToChunk 产出的 part id 同源）
    expect(deltaPartId('tu_1', 'text')).toBe('t-delta-tu_1');
    expect(deltaPartId('tu_1', 'thinking')).toBe('r-delta-tu_1');
    // text / thinking 两类互异（同键双通道无碰撞）
    expect(deltaPartId('tu_1', 'text')).not.toBe(deltaPartId('tu_1', 'thinking'));
    // 不同键互异
    expect(deltaPartId('tu_1', 'text')).not.toBe(deltaPartId('tu_2', 'text'));
    expect(deltaPartId('tu_1', 'thinking')).not.toBe(deltaPartId('tu_2', 'thinking'));
  });

  it('空串键与特殊字符键：part id 生成稳定不炸（None 单通道键形态）', () => {
    // 空串键（provisionalMessageId(null) 的键投影形态）
    expect(deltaPartId('', 'text')).toBe('t-delta-');
    expect(deltaPartId('', 'text')).toBe(deltaPartId('', 'text'));
    // 特殊字符（引号 / 换行 / emoji / 制表）原样拼接不失真
    const weird = ' "q"\n🎉\t';
    expect(deltaPartId(weird, 'text')).toBe(`t-delta-${weird}`);
    expect(deltaPartId(weird, 'thinking')).toBe(`r-delta-${weird}`);
    expect(deltaPartId(weird, 'text')).toBe(deltaPartId(weird, 'text'));
    expect(deltaPartId('', 'text')).not.toBe(deltaPartId(weird, 'text'));
  });
});

describe('provisionalMessageId 边界：空串与特殊字符配对键', () => {
  it('空串配对键：键生成稳定不炸且可区分', () => {
    expect(provisionalMessageId('')).toBe('delta-');
    expect(provisionalMessageId('')).toBe(provisionalMessageId(''));
    expect(provisionalMessageId('')).not.toBe(provisionalMessageId(null));
    expect(provisionalMessageId('')).not.toBe(provisionalMessageId('tu_1'));
  });

  it('含特殊字符配对键：原样拼接稳定不炸', () => {
    const weird = ' "q"\n🎉\t';
    expect(provisionalMessageId(weird)).toBe(`delta-${weird}`);
    expect(provisionalMessageId(weird)).toBe(provisionalMessageId(weird));
    expect(provisionalMessageId(weird)).not.toBe(provisionalMessageId(''));
  });
});

describe('纯 delta 序列折叠：密封-only 重放视图的空态', () => {
  it('纯 delta 序列（无任何密封事件）折叠为空：不炸、不产部件', () => {
    const messages = eventsToUIMessages([
      messageDelta(0, { kind: 'text', text: '增量甲' }),
      messageDelta(1, { kind: 'thinking', thinking: '增量乙' }),
      messageDelta(2, { kind: 'text', text: '增量丙' }),
    ]);

    expect(messages).toEqual([]);
  });
});

describe('runRecordToUIMessage running 形态', () => {
  it('running 轮行（finishedAt null）→ data-run-record 部件逐字段承接：status=running、统计与 finishedAt 全 null', () => {
    const row = record(0, 'running');
    const messages = [runRecordToUIMessage(row)];

    expect(messages).toHaveLength(1);
    expect(messages[0]?.id).toBe('turn-0');
    expect(messages[0]?.role).toBe('system');
    expect(messages[0]?.metadata).toEqual({ seq: null, parentToolUseId: null });
    // data 即 record 整行：running 形态字段面逐字段承接（IPC 信封 Record 臂与
    // 重放轮行两路同构的 running 半边）
    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-run-record');
    expect(part?.type === 'data-run-record' && part.data).toEqual(row);
    expect(part?.type === 'data-run-record' && part.data.status).toBe('running');
    expect(part?.type === 'data-run-record' && part.data.finishedAt).toBeNull();
    expect(part?.type === 'data-run-record' && part.data.sessionId).toBe('s-live');
    expect(part?.type === 'data-run-record' && part.data.numTurns).toBeNull();
    expect(part?.type === 'data-run-record' && part.data.durationMs).toBeNull();
    expect(part?.type === 'data-run-record' && part.data.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// IR 三层（AC-7）：计划层对拍 / 视野差异快照 / 混合配对 / 规模与字符边界 /
// 私有性（公共面仍为 eventsToUIMessages / eventToChunk 两入口）
// ---------------------------------------------------------------------------

/** 同消息场景的四类块形态（计划层对拍矩阵的 fixture 半边）。 */
const SAME_MESSAGE_CASES: Record<string, AgentBlock[]> = {
  'text + thinking': [
    { kind: 'text', text: '结论正文' },
    { kind: 'thinking', thinking: '想一想 🎉' },
  ],
  'toolUse + 同消息 result': [
    { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
    { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
  ],
  '无主 toolResult': [{ kind: 'toolResult', id: 'tu_orphan', content: '孤儿结果', isError: true }],
};

describe('eventsToUIMessages：计划层对拍（同消息场景两路同形）', () => {
  it.each(Object.entries(SAME_MESSAGE_CASES))(
    '%s：折叠路 parts 与实时路 reducer 归约形状相等',
    async (_label, blocks) => {
      const events = [message(7, blocks)];
      const reduced = await reducedMessages(events);

      expect(reduced.map((entry) => entry.id)).toEqual(['evt-7']);
      expect(shapeOf(reduced)).toEqual(shapeOf(eventsToUIMessages(events)));
    },
  );

  it('aux 事件（非 message 密封）两路同形：单 data 部件 / 单 data chunk', async () => {
    const events = [runStarted(0), systemNotice(1), turnDone(2), raw(3)];
    const reduced = await reducedMessages(events);

    expect(shapeOf(reduced)).toEqual(shapeOf(eventsToUIMessages(events)));
  });
});

describe('视野快照：同消息相等（PairingScope 差异不显形）', () => {
  it('单消息内 toolUse + toolResult → 两路均折叠为单卡 output-available（input / output / isError 逐字段）', async () => {
    const events = [
      message(4, [
        { kind: 'toolUse', id: 'tu_9', name: 'Read', input: { path: 'a.ts' } },
        { kind: 'toolResult', id: 'tu_9', content: '文件内容', isError: true },
      ]),
    ];

    const folded = eventsToUIMessages(events);
    expect(folded[0].parts).toHaveLength(1);
    expect(folded[0].parts[0]).toMatchObject({
      type: 'tool-Read',
      toolCallId: 'tu_9',
      state: 'output-available',
      input: { path: 'a.ts' },
      output: { content: '文件内容', isError: true },
    });

    const reduced = await reducedMessages(events);
    expect(shapeOf(reduced)).toEqual(shapeOf(folded));
  });
});

describe('视野快照：跨消息差异锁定（MUST NOT 用单一路径 fixture 掩饰）', () => {
  const crossMessage = (): AgentEvent[] => [
    message(0, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
    message(1, [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }]),
  ];

  it('折叠路（global 视野）：跨消息配对收敛为单卡 output-available，result 消息零部件', () => {
    const folded = eventsToUIMessages(crossMessage());

    expect(folded[0].parts).toHaveLength(1);
    expect(folded[0].parts[0]).toMatchObject({
      type: 'tool-Bash',
      toolCallId: 'tu_1',
      state: 'output-available',
      output: { content: '目录内容', isError: false },
    });
    expect(folded[1].parts).toEqual([]);
  });

  it('实时路（local 视野）：孤儿占位卡 + 后续 result 卡的具体 chunk 序列（能力差异显式锁定）', () => {
    const chunks = crossMessage().flatMap((event) => eventToChunk(event));
    const toolChunks = chunks.filter((chunk) => chunk.type.startsWith('tool-'));

    expect(toolChunks).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'tu_1',
        toolName: 'Bash',
        input: { command: 'ls' },
      },
      {
        type: 'tool-input-available',
        toolCallId: 'tu_1',
        toolName: '工具调用',
        input: null,
      },
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: '目录内容', isError: false },
      },
    ]);
  });

  it('两路差异显式可辨：result 消息在折叠路零部件、在实时路 reducer 侧仍有 tool 部件', async () => {
    const events = crossMessage();
    const folded = eventsToUIMessages(events);
    const reduced = await reducedMessages(events);

    expect(folded[1].parts).toEqual([]);
    const localSecond = reduced.find((entry) => entry.id === 'evt-1');
    expect(localSecond?.parts.length ?? 0).toBeGreaterThan(0);
  });
});

describe('eventsToUIMessages：多 toolUse / result 混合配对', () => {
  it('同 id 重复 result 取首个；result 在 use 之前的同消息仍配对收敛单卡', () => {
    const events = [
      message(0, [
        { kind: 'toolResult', id: 'tu_a', content: '首个结果', isError: false },
        { kind: 'toolUse', id: 'tu_a', name: 'Bash', input: { command: 'pwd' } },
        { kind: 'toolUse', id: 'tu_b', name: 'Read', input: { path: 'b.ts' } },
      ]),
      message(1, [
        { kind: 'toolResult', id: 'tu_a', content: '重复结果被丢弃', isError: true },
        { kind: 'toolResult', id: 'tu_b', content: 'b 的结果', isError: false },
      ]),
    ];

    const folded = eventsToUIMessages(events);
    const parts = folded[0].parts;
    expect(parts).toHaveLength(2);
    expect(parts[0]).toMatchObject({
      type: 'tool-Bash',
      toolCallId: 'tu_a',
      state: 'output-available',
      output: { content: '首个结果', isError: false },
    });
    expect(parts[1]).toMatchObject({
      type: 'tool-Read',
      state: 'output-available',
      output: { content: 'b 的结果', isError: false },
    });
    // 两个 result 均已被消费：第二消息零部件（无双卡 / 无孤儿）
    expect(folded[1].parts).toEqual([]);
  });
});

describe('eventsToUIMessages：规模与字符边界', () => {
  it('1000 条密封事件不抛错、输出保序且 metadata.seq 单调', () => {
    const events: AgentEvent[] = [];
    for (let seq = 0; seq < 1000; seq += 1) {
      events.push(message(seq, [{ kind: 'text', text: `第 ${seq} 段` }]));
    }

    const folded = eventsToUIMessages(events);

    expect(folded).toHaveLength(1000);
    expect(folded.map((entry) => entry.id)).toEqual(events.map((event) => `evt-${event.seq}`));
    const seqs = folded.map((entry) => entry.metadata?.seq ?? -1);
    expect(seqs).toEqual(Array.from({ length: 1000 }, (_unused, index) => index));
  });

  it('超长文本（> 1000 chars）与特殊字符（换行 / NUL / emoji）逐字保留', () => {
    const longText = '超'.repeat(1200);
    const specialText = '第一行\n第二行\u{0}结束😀🎉';
    const folded = eventsToUIMessages([
      message(0, [{ kind: 'text', text: longText }]),
      message(1, [{ kind: 'text', text: specialText }]),
    ]);

    expect(folded[0].parts[0]).toMatchObject({ type: 'text', text: longText });
    expect(folded[1].parts[0]).toMatchObject({ type: 'text', text: specialText });
  });
});

describe('eventToChunk：密封路发射形（IR 化后）', () => {
  it('部件 id 为计划下标形（t-<seq>-<i> / r-<seq>-<i>），工具走 tool-input-available / tool-output-available', () => {
    const chunks = eventToChunk(
      message(5, [
        { kind: 'thinking', thinking: '先想' },
        { kind: 'text', text: '再答' },
        { kind: 'toolUse', id: 'tu_x', name: 'Glob', input: { pattern: '*.ts' } },
        { kind: 'toolResult', id: 'tu_x', content: '命中 2 个', isError: false },
      ]),
    );

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      'start',
      'reset-step',
      'reasoning-start',
      'reasoning-delta',
      'reasoning-end',
      'text-start',
      'text-delta',
      'text-end',
      'tool-input-available',
      'tool-output-available',
    ]);
    expect(chunks[2]).toMatchObject({ type: 'reasoning-start', id: 'r-5-0' });
    expect(chunks[5]).toMatchObject({ type: 'text-start', id: 't-5-1' });
    expect(chunks[8]).toMatchObject({
      type: 'tool-input-available',
      toolCallId: 'tu_x',
      toolName: 'Glob',
      input: { pattern: '*.ts' },
    });
    expect(chunks[9]).toMatchObject({
      type: 'tool-output-available',
      toolCallId: 'tu_x',
      output: { content: '命中 2 个', isError: false },
    });
  });

  it('aux 路：start + reset-step + 单 data 部件（载荷与折叠路同事件逐字段相等）', () => {
    const event = turnDone(9);
    const chunks = eventToChunk(event);
    const folded = eventsToUIMessages([event]);

    expect(chunks.map((chunk) => chunk.type)).toEqual(['start', 'reset-step', 'data-run-result']);
    expect(chunks[2]).toEqual(folded[0].parts[0]);
  });

  it('未知 aux 事件（未来变体）→ start + reset-step + data-raw，chunk 流不中断', () => {
    const future = {
      seq: 3,
      timestampMs: TS,
      kind: 'futureVariant',
      note: '未来字段',
    } as unknown as AgentEvent;

    const chunks = eventToChunk(future);

    expect(chunks.map((chunk) => chunk.type)).toEqual(['start', 'reset-step', 'data-raw']);
    expect(chunks[2]).toMatchObject({
      type: 'data-raw',
      data: { seq: 3, eventType: 'unknown' },
    });
  });
});

describe('runRecordToUIMessage：异常与边界', () => {
  it('未来 / 非法 status 与未知多余字段原样透传（不解释、不丢字段）', () => {
    const future = {
      ...record(5, 'completed'),
      status: 'futureStatus',
      futureColumn: '未来列',
      error: '第一行\n第二行😀',
    } as unknown as TurnSummary;

    const message = runRecordToUIMessage(future);
    const part = message.parts[0] as { type: string; data: Record<string, unknown> };

    expect(part.type).toBe('data-run-record');
    expect(part.data.status).toBe('futureStatus');
    expect(part.data.futureColumn).toBe('未来列');
    expect(part.data.error).toBe('第一行\n第二行😀');
  });

  it('turnId 两极（0 / -1 / 超大整数）→ 消息 id `turn-<id>` 稳定且互异', () => {
    const ids = [0, -1, Number.MAX_SAFE_INTEGER].map(
      (turnId) => runRecordToUIMessage(record(turnId, 'completed')).id,
    );
    expect(ids).toEqual(['turn-0', 'turn--1', `turn-${Number.MAX_SAFE_INTEGER}`]);
    expect(new Set(ids).size).toBe(3);
  });

  it('统计字段全 null 形态可折叠（metadata.seq 恒 null）', () => {
    const message = runRecordToUIMessage({
      turnId: 6,
      sessionId: 'ses-x',
      status: 'running',
      startedAt: TS,
      finishedAt: null,
      numTurns: null,
      costUsd: null,
      durationMs: null,
      error: null,
    });

    expect(message.metadata?.seq).toBeNull();
    expect(message.parts[0]).toMatchObject({
      type: 'data-run-record',
      data: {
        turnId: 6,
        status: 'running',
        finishedAt: null,
        costUsd: null,
        error: null,
      },
    });
  });
});

describe('provisionalMessageId / deltaPartId：线格式与互不碰撞', () => {
  it('空串键与 null 键可区分（delta- ≠ delta-none），特殊字符键稳定不抛错', () => {
    expect(provisionalMessageId(null)).toBe('delta-none');
    expect(provisionalMessageId('')).toBe('delta-');
    expect(provisionalMessageId('')).not.toBe(provisionalMessageId(null));
    expect(provisionalMessageId('tu\n😀\u{0}')).toBe('delta-tu\n😀\u{0}');
  });

  it('超长键与确定性：多次调用恒等且 text / thinking 两族互异', () => {
    const longKey = 'k'.repeat(2000);
    expect(deltaPartId(longKey, 'text')).toBe(deltaPartId(longKey, 'text'));
    expect(deltaPartId(longKey, 'text')).not.toBe(deltaPartId(longKey, 'thinking'));
    expect(deltaPartId('', 'text')).toBe('t-delta-');
    expect(deltaPartId('', 'thinking')).toBe('r-delta-');
  });
});

describe('IR 三层私有性（D11）：公共面仍为两入口', () => {
  it('模块运行时导出键集合锁定（BlockPlan / planBlock / plansToParts / plansToChunks / PairingScope 不外露）', () => {
    expect(Object.keys(adapterModule).sort()).toEqual([
      'deltaPartId',
      'eventToChunk',
      'eventsToUIMessages',
      'provisionalMessageId',
      'runRecordToUIMessage',
    ]);
    for (const internal of [
      'BlockPlan',
      'planBlock',
      'plansToParts',
      'plansToChunks',
      'PairingScope',
      'auxEventData',
    ]) {
      expect(adapterModule).not.toHaveProperty(internal);
    }
  });
});

// ---------------------------------------------------------------------------
// web_search 服务端工具块映射（AC-9）：工具卡管线承载 + 两视野配对
// ---------------------------------------------------------------------------

/** `web_search` 来源 fixture（serde camelCase 线格式）。 */
function searchSource(url: string, title: string | null = null) {
  return { url, title, pageAge: null };
}

/** use 侧 `webSearch` 块（`rawUse != null` 即 use 侧）。 */
function searchUseBlock(
  id: string,
  query: string,
  rawResult: unknown = null,
  results: ReturnType<typeof searchSource>[] = [],
): AgentBlock {
  return {
    kind: 'webSearch',
    id,
    query,
    results,
    error: null,
    rawUse: { type: 'server_tool_use', id, name: 'web_search', input: { query } },
    rawResult,
  };
}

/** result 侧 `webSearch` 块（`rawUse == null` 即 result 侧）。 */
function searchResultBlock(
  id: string,
  results: ReturnType<typeof searchSource>[],
  error: string | null = null,
): AgentBlock {
  return {
    kind: 'webSearch',
    id,
    query: '',
    results,
    error,
    rawUse: null,
    rawResult: { type: 'web_search_tool_result', tool_use_id: id },
  };
}

describe('agent-adapter：web_search 未配对 use 侧与同消息配对态', () => {
  it('未配对 use 侧 → 单个 tool-web_search 部件、input-available、input 深等于 { query }、无 output 键', () => {
    const messages = eventsToUIMessages([message(1, [searchUseBlock('srvtoolu_1', '今日汇率')])]);

    const part = messages[0]?.parts[0];
    expect(part).toEqual({
      type: 'tool-web_search',
      toolCallId: 'srvtoolu_1',
      state: 'input-available',
      input: { query: '今日汇率' },
    });
    expect(part !== undefined && 'output' in part).toBe(false);
    // AgentDataParts 成员零新增：不产任何 data-* 部件
    expect(messages[0]?.parts.every((entry) => !entry.type.startsWith('data-'))).toBe(true);
  });

  it('use 块自带 rawResult（pause_turn 未拆分）→ output-available 且 output 为加位形态', () => {
    const results = [searchSource('https://a/1', '甲'), searchSource('https://b/2')];
    const messages = eventsToUIMessages([
      message(1, [searchUseBlock('srvtoolu_2', '查询词', { type: 'x' }, results)]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('tool-web_search');
    expect(part?.type === 'tool-web_search' && part.state).toBe('output-available');
    expect(part?.type === 'tool-web_search' && part.output).toEqual({
      content: '',
      isError: false,
      search: { results, error: null },
    });
  });

  it('error 在场 → isError 为 true、search.error 为错误码、search.results 为空数组（非 null）', () => {
    // 错误载荷在 result 侧块：use 侧不带自身 rawResult，故 output 取配对命中
    const messages = eventsToUIMessages([
      message(1, [searchUseBlock('srvtoolu_err', '超限')]),
      message(2, [searchResultBlock('srvtoolu_err', [], 'max_uses_exceeded')]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'tool-web_search' && part.output).toEqual({
      content: '',
      isError: true,
      search: { results: [], error: 'max_uses_exceeded' },
    });
  });
});

describe('agent-adapter：web_search 跨消息配对与孤儿 result 侧', () => {
  it('消息 A 携 use、消息 B 携同 id result → 该 id 收敛为单个 tool 部件（result 侧不重复呈现）', () => {
    const messages = eventsToUIMessages([
      message(1, [searchUseBlock('srvtoolu_x', '跨消息查询')]),
      message(2, [searchResultBlock('srvtoolu_x', [searchSource('https://x/1', '跨消息来源')])]),
    ]);

    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-web_search',
        toolCallId: 'srvtoolu_x',
        state: 'output-available',
        input: { query: '跨消息查询' },
        output: {
          content: '',
          isError: false,
          search: { results: [searchSource('https://x/1', '跨消息来源')], error: null },
        },
      },
    ]);
    expect(messages[1]?.parts).toEqual([]);
  });

  it('孤儿 result 侧（无配对 use）→ input 为 null、output.search 携来源、toolCallId 取块 id', () => {
    const messages = eventsToUIMessages([
      message(3, [searchResultBlock('srvtoolu_orphan', [searchSource('https://o/1', '孤儿来源')])]),
    ]);

    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-web_search',
        toolCallId: 'srvtoolu_orphan',
        state: 'output-available',
        input: null,
        output: {
          content: '',
          isError: false,
          search: { results: [searchSource('https://o/1', '孤儿来源')], error: null },
        },
      },
    ]);
  });

  it('两视野差异显式锁定：折叠路单卡 output-available；实时路 input-available + 孤儿 output-available', () => {
    const events = [
      message(1, [searchUseBlock('srvtoolu_view', '视野查询')]),
      message(2, [searchResultBlock('srvtoolu_view', [searchSource('https://v/1')])]),
    ];

    // 折叠路（global 视野）
    const folded = eventsToUIMessages(events);
    expect(folded[0]?.parts).toHaveLength(1);
    expect(folded[0]?.parts[0]?.type === 'tool-web_search' && folded[0]?.parts[0]?.state).toBe(
      'output-available',
    );
    expect(folded[1]?.parts).toEqual([]);

    // 实时路（local 视野：仅当前 message 内配对）
    const toolChunks = events
      .flatMap((event) => eventToChunk(event))
      .filter((chunk) => chunk.type.startsWith('tool-'));
    expect(toolChunks).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'srvtoolu_view',
        toolName: 'web_search',
        input: { query: '视野查询' },
      },
      {
        type: 'tool-input-available',
        toolCallId: 'srvtoolu_view',
        toolName: 'web_search',
        input: null,
      },
      {
        type: 'tool-output-available',
        toolCallId: 'srvtoolu_view',
        output: {
          content: '',
          isError: false,
          search: { results: [searchSource('https://v/1')], error: null },
        },
      },
    ]);

    // 同消息配对时两入口产物相等
    const sameMessage = [message(4, [searchUseBlock('srvtoolu_same', '同消息查询')])];
    const sameFolded = eventsToUIMessages(sameMessage);
    const sameChunks = eventToChunk(sameMessage[0]);
    expect(
      sameChunks.filter(
        (chunk) => chunk.type === 'tool-input-available' || chunk.type === 'tool-output-available',
      ),
    ).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'srvtoolu_same',
        toolName: 'web_search',
        input: { query: '同消息查询' },
      },
    ]);
    expect(sameFolded[0]?.parts).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// text 块 citations → providerMetadata.agent.citations（AC-9 / D12）
// ---------------------------------------------------------------------------

/** 引用 fixture（`raw` 为原始 wire 值）。 */
function citation(url: string | null, title: string | null = null) {
  return { url, title, citedText: null, raw: { url, title } };
}

describe('agent-adapter：text 部件 citations metadata', () => {
  it('两条同 url 引用原样进部件（去重责任在呈现层），metadata 深等于 { agent: { citations } }', () => {
    const citations = [citation('https://dup/1', '甲'), citation('https://dup/1', '乙')];
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'text', text: '带引用正文', citations }]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'text' && part.providerMetadata).toEqual({ agent: { citations } });
    // 命名空间为 agent：不存在 anthropic 键
    expect(part?.type === 'text' && part.providerMetadata).not.toHaveProperty('anthropic');
  });

  it('citations 为空数组 / 键缺席 / null → providerMetadata 为 undefined（不挂空壳）', () => {
    const cases: AgentBlock[] = [
      { kind: 'text', text: '空数组', citations: [] },
      { kind: 'text', text: '键缺席' },
      { kind: 'text', text: 'null', citations: null },
    ];

    for (const block of cases) {
      const part = eventsToUIMessages([message(1, [block])])[0]?.parts[0];
      expect(part?.type === 'text' && part.providerMetadata).toBeUndefined();
    }
  });

  it('chunk 路：text-start 与 text-delta 带同一份 metadata，text-end 不带；经 reducer 归约后与折叠路同形', async () => {
    const citations = [citation('https://cite/1', '引用标题')];
    const events: AgentEvent[] = [message(1, [{ kind: 'text', text: 'chunk 正文', citations }])];

    const chunks = eventToChunk(events[0]);
    const start = chunks.find((chunk) => chunk.type === 'text-start');
    const delta = chunks.find((chunk) => chunk.type === 'text-delta');
    const end = chunks.find((chunk) => chunk.type === 'text-end');

    expect(start?.type === 'text-start' && start.providerMetadata).toEqual({
      agent: { citations },
    });
    expect(delta?.type === 'text-delta' && delta.providerMetadata).toEqual({
      agent: { citations },
    });
    expect(end?.type === 'text-end' && end.providerMetadata).toBeUndefined();
    // 同一份对象（引用相等）：metadata 合并语义为覆盖非累加
    expect(start?.type === 'text-start' ? start.providerMetadata : null).toBe(
      delta?.type === 'text-delta' ? delta.providerMetadata : undefined,
    );

    // 折叠路同形
    const foldedPart = eventsToUIMessages(events)[0]?.parts[0];
    const reduced = await reducedMessages(events);
    expect(reduced[0]?.parts[0]?.type === 'text' && reduced[0]?.parts[0]?.providerMetadata).toEqual(
      foldedPart?.type === 'text' ? foldedPart.providerMetadata : undefined,
    );
  });

  it('chunk 路空 citations：三组 chunk 均不带 metadata', () => {
    const chunks = eventToChunk(message(1, [{ kind: 'text', text: '无引用', citations: [] }]));

    for (const chunk of chunks) {
      if (chunk.type === 'text-start' || chunk.type === 'text-delta' || chunk.type === 'text-end') {
        expect(chunk.providerMetadata).toBeUndefined();
      }
    }
  });
});

// ---------------------------------------------------------------------------
// web_search 计划层边界与整体映射（组合 / 异常 / 边界）
// ---------------------------------------------------------------------------

describe('agent-adapter：web_search 取值边界与未知变体', () => {
  it('query 空串 / 超长 / 含 emoji 与换行、results 空与 100 条均产出部件不 panic', () => {
    const longQuery = `${'查'.repeat(1200)}\nemoji 🎉`;
    const manySources = Array.from({ length: 100 }, (_, index) =>
      searchSource(`https://many/${index}`, `来源 ${index}`),
    );

    for (const [query, results] of [
      ['', []],
      [longQuery, manySources],
    ] as [string, ReturnType<typeof searchSource>[]][]) {
      const messages = eventsToUIMessages([
        message(1, [searchUseBlock('srvtoolu_edge', query, { type: 'x' }, results)]),
      ]);
      const part = messages[0]?.parts[0] as unknown as AgentToolPart;
      expect(part.type).toBe('tool-web_search');
      expect(part.input).toEqual({ query });
      expect(part.state).toBe('output-available');
      expect(part.state === 'output-available' ? part.output.search?.results.length : -1).toBe(
        results.length,
      );
    }
  });

  it('rawUse 为空对象（非 null）仍判 use 侧：查询词取自块 query 字段而非 rawUse', () => {
    const messages = eventsToUIMessages([
      message(1, [{ kind: 'webSearch', id: 'srvtoolu_obj', query: '块字段查询词', rawUse: {} }]),
    ]);

    expect(messages[0]?.parts[0]).toEqual({
      type: 'tool-web_search',
      toolCallId: 'srvtoolu_obj',
      state: 'input-available',
      input: { query: '块字段查询词' },
    });
  });

  it('线格式四键全缺省形态按 rawUse 值判 result 侧：无配对产孤儿 plan、有配对被消费', () => {
    // D1 第四稿：缺省键恒在（results: [] / error: null / rawUse: null / rawResult: null），
    // 判别必须取值而非「键是否存在」
    const defaults = {
      kind: 'webSearch',
      id: 'srvtoolu_defaults',
      query: '',
      results: [],
      error: null,
      rawUse: null,
      rawResult: null,
    } as AgentBlock;

    // 无配对 → 孤儿 plan（input: null，载荷不丢）
    expect(eventsToUIMessages([message(1, [defaults])])[0]?.parts).toEqual([
      {
        type: 'tool-web_search',
        toolCallId: 'srvtoolu_defaults',
        state: 'output-available',
        input: null,
        output: { content: '', isError: false, search: { results: [], error: null } },
      },
    ]);

    // 有配对 → 被 use 消费、返回 null（不重复呈现）
    const paired = eventsToUIMessages([
      message(2, [searchUseBlock('srvtoolu_defaults', '配对查询')]),
      message(3, [defaults]),
    ]);
    expect(paired[0]?.parts).toHaveLength(1);
    expect(paired[1]?.parts).toEqual([]);
  });

  it('运行时未知块变体（未来新增）→ 不产部件、不抛错、不产 data-raw 兜底', () => {
    const unknownBlock = { kind: 'futureBlock', text: '未知' } as unknown as AgentBlock;
    const messages = eventsToUIMessages([message(1, [unknownBlock])]);

    expect(messages[0]?.parts).toEqual([]);
  });

  it('五变体事件序列整体映射：块序保原序、搜索两态可辨、data 部件成员零新增', () => {
    const events: AgentEvent[] = [
      runStarted(0),
      message(2, [
        { kind: 'text', text: '带引用正文', citations: [citation('https://cite/1')] },
        { kind: 'thinking', thinking: '先想一下' },
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
        searchUseBlock('srvtoolu_pair', '已配对查询'),
        searchUseBlock('srvtoolu_wait', '未配对查询'),
      ]),
      message(3, [
        searchResultBlock('srvtoolu_pair', [searchSource('https://pair/1', '配对来源')]),
      ]),
      systemNotice(4),
      turnDone(5),
      raw(6),
    ];

    const messages = eventsToUIMessages(events);
    const assistantParts = messages[1]?.parts ?? [];

    expect(assistantParts.map((part) => part.type)).toEqual([
      'text',
      'reasoning',
      'tool-Bash',
      'tool-web_search',
      'tool-web_search',
    ]);
    // 搜索两态：已配对 output-available、未配对 input-available
    expect(assistantParts[3]?.type === 'tool-web_search' && assistantParts[3]?.state).toBe(
      'output-available',
    );
    expect(assistantParts[4]?.type === 'tool-web_search' && assistantParts[4]?.state).toBe(
      'input-available',
    );
    // AgentDataParts 成员零新增：无 data-citations / data-web-search 部件
    const partTypes = messages.flatMap((entry) => entry.parts.map((part) => part.type));
    expect(partTypes).not.toContain('data-citations');
    expect(partTypes).not.toContain('data-web-search');
    expect(partTypes.filter((type) => type.startsWith('data-')).sort()).toEqual([
      'data-raw',
      'data-run-result',
      'data-run-started',
      'data-system-notice',
    ]);
  });
});
