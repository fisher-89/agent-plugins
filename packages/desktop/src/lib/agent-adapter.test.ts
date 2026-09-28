import { useChat } from '@ai-sdk/react';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ChatTransport } from 'ai';
import { describe, expect, it } from 'vite-plus/test';

import type { AgentBlock, AgentEvent, AgentRunRecord } from '../types/dto';
import {
  eventToChunk,
  eventsToUIMessages,
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

function runResult(
  seq: number,
  fields: Partial<Extract<AgentEvent, { kind: 'runResult' }>> = {},
): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runResult',
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

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery","note":"保留原文"}',
  };
}

function record(id: number, status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id,
    prompt: '调试一轮',
    cwd: 'C:\\demo\\alpha',
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: TS,
    finishedAt: status === 'running' ? null : TS + 999,
    numTurns: status === 'running' ? null : 2,
    costUsd: status === 'running' ? null : 0.2,
    durationMs: status === 'running' ? null : 800,
    sessionId: status === 'running' ? null : 's-done',
    error: status === 'failed' ? '进程结束但未产出 result 事件' : null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
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

  it('runResult 产 data-run-result 部件：subtype / usage 等汇总载荷保真', () => {
    const messages = eventsToUIMessages([runResult(3)]);

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

  it('role 为其他字符串的 message 事件收窄为 assistant（非 user 即 assistant）', () => {
    const odd = { ...message(5, []), role: 'tool' } as AgentEvent;
    const messages = eventsToUIMessages([odd]);

    expect(messages[0]?.role).toBe('assistant');
  });

  it('非 message 事件各产出一条 system 角色消息携带单个 data 部件，seq 保序', () => {
    const events = [runStarted(0), systemNotice(1), runResult(2), raw(3)];
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
    expect(messages[0]?.id).toBe('run-11');
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
      runResult(5),
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
