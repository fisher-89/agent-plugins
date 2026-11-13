import { useChat } from '@ai-sdk/react';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ChatTransport } from 'ai';
import { describe, expect, expectTypeOf, it } from 'vite-plus/test';

import type { AgentBlock, AgentEvent, TurnSummary } from '../types/dto';
import * as adapterModule from './agent-adapter';
import {
  deltaPartId,
  eventToChunk,
  foldSessionMessages,
  type AgentToolPart,
  type AgentUIMessage,
  type AgentUIMessageChunk,
  type DeltaPartAnchor,
} from './agent-adapter';

// ---------------------------------------------------------------------------
// 纯函数模块，无外部依赖（`ai` 仅类型引用），不需要 Mock。
// fixture 对齐 serde camelCase 线格式（types/dto 镜像）。
// ---------------------------------------------------------------------------

const TS = 1727000000000;
/** 多数用例只见一轮：分组键与消息 id 锚点都由它派生（`turn-<turnId>`） */
const TURN = 11;

function runStarted(seq: number, turnId: number | null = TURN): AgentEvent {
  return {
    seq,
    turnId,
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
  turnId: number | null = TURN,
): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks,
    parentToolUseId,
  };
}

function toolMessage(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
  turnId: number | null = TURN,
): AgentEvent {
  return { ...message(seq, blocks, parentToolUseId, turnId), role: 'tool' } as AgentEvent;
}

function userMessage(seq: number, text: string, turnId: number | null = TURN): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function systemNotice(seq: number, turnId: number | null = TURN): AgentEvent {
  return {
    seq,
    turnId,
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
    turnId: TURN,
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
  turnId: number | null = TURN,
): AgentEvent {
  return { seq, turnId, timestampMs: TS, kind: 'messageDelta', parentToolUseId, delta };
}

function raw(seq: number, turnId: number | null = TURN): AgentEvent {
  return {
    seq,
    turnId,
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

/** 缺省单行轮行（多数用例只关心折叠形状，不关心轮行明细） */
function fold(events: AgentEvent[], turns: TurnSummary[] = [record(TURN, 'completed')]) {
  return foldSessionMessages(events, turns);
}

/** 该轮 assistant 消息（恒一条；缺失即 `undefined`） */
function assistantOf(messages: AgentUIMessage[]): AgentUIMessage | undefined {
  return messages.find((message) => message.role === 'assistant');
}

// ---------------------------------------------------------------------------
// foldSessionMessages：五变体映射（aux → 逐事件独立 system 消息）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：五变体映射', () => {
  it('runStarted 产 data-run-started 部件：model / sessionId / tools / mcpServers 载荷保真', () => {
    const messages = fold([runStarted(0)]);

    const part = messages[0]?.parts[0];
    expect(messages[0]?.role).toBe('system');
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
    const messages = fold([
      message(1, [
        { kind: 'thinking', thinking: '先想一想' },
        { kind: 'text', text: '正文内容' },
      ]),
    ]);

    expect(assistantOf(messages)?.parts).toEqual([
      { type: 'reasoning', text: '先想一想', state: 'done' },
      { type: 'text', text: '正文内容', state: 'done' },
    ]);
  });

  it('message.toolUse 产 tool 部件：type=tool-<name> 且 toolCallId=block.id、input 保真', () => {
    const messages = fold([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
    ]);

    expect(assistantOf(messages)?.parts).toEqual([
      {
        type: 'tool-Bash',
        toolCallId: 'tu_1',
        state: 'input-available',
        input: { command: 'ls' },
      },
    ]);
  });

  it('systemNotice 产 data-system-notice 部件：subtype 与 payload 载荷保真', () => {
    const messages = fold([systemNotice(2)]);

    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-system-notice');
    expect(part?.type === 'data-system-notice' && part.data).toEqual({
      seq: 2,
      timestampMs: TS,
      subtype: 'permission_denial',
      payload: { tool: 'Bash' },
    });
  });

  it('turnDone 产 data-run-result 部件：subtype / usage 等汇总载荷保真（与 metadata.record 两个层级）', () => {
    const messages = fold([turnDone(3)]);

    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-run-result');
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
    const messages = fold([raw(4)]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-raw' && part.data).toEqual({
      seq: 4,
      timestampMs: TS,
      model: undefined,
      eventType: 'mystery',
      rawJson: '{"type":"mystery","note":"保留原文"}',
    });
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：消息形状约定（id 锚点 / metadata / 角色 / 轮内序）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：消息形状约定', () => {
  it('一轮产出提问 + 一条 assistant：id 为 turn-<turnId>-user / turn-<turnId>，metadata 携 seq 与 parentToolUseId', () => {
    const messages = fold([
      userMessage(0, '帮我查'),
      message(1, [{ kind: 'text', text: '好的' }], 'tu_9'),
    ]);

    expect(messages.map((m) => m.id)).toEqual(['turn-11-user', 'turn-11']);
    expect(messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(messages[0]?.metadata).toMatchObject({ seq: 0, parentToolUseId: null });
    expect(messages[1]?.metadata).toMatchObject({ seq: 0, parentToolUseId: 'tu_9' });
  });

  it('tool role（工具结果管道）并入该轮 assistant 消息，不产独立 system 消息', () => {
    const messages = fold([
      message(4, [{ kind: 'toolUse', id: 'tu_1', name: 'read', input: {} }]),
      toolMessage(5, [{ kind: 'toolResult', id: 'tu_1', content: 'ok', isError: false }]),
    ]);

    expect(messages.map((m) => m.role)).toEqual(['assistant']);
    // 已配对结果就地补全工具卡（input 与 output 同住一条消息）
    expect(assistantOf(messages)?.parts).toEqual([
      {
        type: 'tool-read',
        toolCallId: 'tu_1',
        state: 'output-available',
        input: {},
        output: { content: 'ok', isError: false },
      },
    ]);
  });

  it('非 message 事件各产出一条 system 角色消息携带单个 data 部件，seq 保序且 assistant 为该轮末条', () => {
    const events = [runStarted(0), systemNotice(1), turnDone(2), raw(3)];
    const messages = fold(events);

    expect(messages.map((m) => m.id)).toEqual(['evt-0', 'evt-1', 'evt-2', 'evt-3', 'turn-11']);
    expect(messages.map((m) => m.role)).toEqual([
      'system',
      'system',
      'system',
      'system',
      'assistant',
    ]);
    expect(messages.map((m) => m.parts.map((part) => part.type))).toEqual([
      ['data-run-started'],
      ['data-system-notice'],
      ['data-run-result'],
      ['data-raw'],
      [],
    ]);
  });

  it('assistant 恒为该轮末条（isContinuation 成立的硬前提：其后不跟任何 aux 消息）', () => {
    const messages = fold([runStarted(0), userMessage(1, '问'), message(2, []), raw(3)]);

    expect(messages.at(-1)?.id).toBe('turn-11');
    expect(messages.at(-1)?.role).toBe('assistant');
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：工具同 id 配对
// ---------------------------------------------------------------------------

describe('foldSessionMessages：工具同 id 配对', () => {
  it('toolUse 块与跨消息同 id toolResult 合并为单 tool 部件（input + output-available 形态）', () => {
    const messages = fold([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
      toolMessage(2, [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }]),
    ]);

    expect(messages).toHaveLength(1);
    expect(messages[0]?.parts).toEqual([
      {
        type: 'tool-Bash',
        toolCallId: 'tu_1',
        state: 'output-available',
        input: { command: 'ls' },
        output: { content: '目录内容', isError: false },
      },
    ]);
  });

  it('isError 的 result 数据随配对并入 output', () => {
    const messages = fold([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: {} }]),
      toolMessage(2, [{ kind: 'toolResult', id: 'tu_1', content: 'boom', isError: true }]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'tool-Bash' && part.output).toEqual({
      content: 'boom',
      isError: true,
    });
  });

  it('重复 id 的 result 取首个，不重复续 output', () => {
    const messages = fold([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: {} }]),
      toolMessage(2, [{ kind: 'toolResult', id: 'tu_1', content: '首个结果', isError: false }]),
      toolMessage(3, [{ kind: 'toolResult', id: 'tu_1', content: '迟到结果', isError: false }]),
    ]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'tool-Bash' && part.output).toEqual({
      content: '首个结果',
      isError: false,
    });
    expect(messages[0]?.parts).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：无主 toolResult（边界）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：无主 toolResult', () => {
  it('无配对 toolUse 的 toolResult 就地合成占位部件（name=工具调用），result 数据不丢', () => {
    const messages = fold([
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
// foldSessionMessages：空输入与空内容轮（边界）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：空输入与空内容轮', () => {
  it('空事件数组 + 空轮行 → 空消息序列', () => {
    expect(foldSessionMessages([], [])).toEqual([]);
  });

  it('空事件数组 + 三行轮行 → 恰三条 assistant（各一 parts 空、record 就位、seq null），无提问与 aux', () => {
    const turns = [record(31, 'completed'), record(32, 'stopped'), record(33, 'failed')];

    const messages = foldSessionMessages([], turns);

    expect(messages).toHaveLength(3);
    expect(messages.map((message) => message.id)).toEqual(['turn-31', 'turn-32', 'turn-33']);
    expect(messages.map((message) => message.role)).toEqual([
      'assistant',
      'assistant',
      'assistant',
    ]);
    expect(messages.map((message) => message.parts)).toEqual([[], [], []]);
    expect(messages.map((message) => message.metadata?.seq)).toEqual([null, null, null]);
    expect(messages.map((message) => message.metadata?.record)).toEqual(turns);
  });

  it('blocks 为空数组的 message 事件仍产出无部件的 assistant 消息（消息不丢）', () => {
    const messages = fold([message(0, [])]);

    expect(messages).toHaveLength(1);
    expect(messages[0]?.id).toBe('turn-11');
    expect(messages[0]?.parts).toEqual([]);
  });

  it('零事件轮（有轮行、事件侧零事件）仍产 parts 空的 assistant，record 就位、seq 为 null', () => {
    const messages = foldSessionMessages([], [record(7, 'stopped')]);

    expect(messages).toHaveLength(1);
    expect(messages[0]).toMatchObject({
      id: 'turn-7',
      role: 'assistant',
      metadata: { seq: null, parentToolUseId: null, record: record(7, 'stopped') },
      parts: [],
    });
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：未知事件透传（边界）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：未知事件透传', () => {
  it('raw 变体（未知 event_type）→ data-raw 部件原文透传，原文 JSON 不被解释改写', () => {
    const original = '{"type":"mystery","nested":{"n":1},"note":"第一行\\n中文 🎉"}';
    const event: AgentEvent = {
      seq: 7,
      turnId: TURN,
      timestampMs: TS,
      kind: 'raw',
      eventType: 'totally-unknown',
      rawJson: original,
    };
    const messages = fold([event]);

    const part = messages[0]?.parts[0];
    expect(part?.type === 'data-raw' && part.data.rawJson).toBe(original);
  });

  it('运行时未知事件变体（未来 CLI 新增）→ data-raw 整事件 JSON 透传不丢', () => {
    const timeTravel = {
      seq: 9,
      turnId: TURN,
      timestampMs: TS,
      kind: 'timeTravel',
      targetEpoch: 2,
    } as unknown as AgentEvent;
    const messages = fold([timeTravel]);

    const part = messages[0]?.parts[0];
    expect(part?.type).toBe('data-raw');
    expect(part?.type === 'data-raw' && part.data.rawJson).toBe(JSON.stringify(timeTravel));
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：record 归 metadata（取代 data-run-record 部件）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：record 归 metadata', () => {
  it('轮行按 turnId 匹配后挂该轮 assistant 的 metadata.record，不产 data-run-record 部件', () => {
    const row = record(11, 'completed');
    const messages = fold([message(1, [{ kind: 'text', text: '答' }])], [row]);

    expect(assistantOf(messages)?.metadata?.record).toEqual(row);
    const partTypes = messages.flatMap((message) => message.parts.map((part) => part.type));
    expect(partTypes).not.toContain('data-run-record');
  });

  it('completed / failed / stopped 三态的 status、error、finishedAt、sessionId 均可在 metadata.record 中考证', () => {
    const rows = [record(1, 'completed'), record(2, 'failed'), record(3, 'stopped')];
    const records = rows.map((row) => {
      const messages = foldSessionMessages([], [row]);
      return messages[0]?.metadata?.record;
    });

    expect(records.map((entry) => entry?.status)).toEqual(['completed', 'failed', 'stopped']);
    expect(records[1]?.error).toBe('进程结束但未产出收敛事件');
    expect(records[2]?.error).toBeNull();
    expect(records[0]?.finishedAt).toBe(TS + 999);
    expect(records[0]?.sessionId).toBe('s-done');
  });

  it('running 轮行（finishedAt null）逐字段承接：status=running、统计与 finishedAt 全 null', () => {
    const row = record(0, 'running');
    const messages = foldSessionMessages([], [row]);

    expect(messages[0]?.id).toBe('turn-0');
    expect(messages[0]?.metadata).toMatchObject({
      seq: null,
      parentToolUseId: null,
      record: row,
    });
    expect(messages[0]?.metadata?.record?.finishedAt).toBeNull();
    expect(messages[0]?.metadata?.record?.numTurns).toBeNull();
    expect(messages[0]?.metadata?.record?.durationMs).toBeNull();
  });

  it('未来 / 非法 status 与未知多余字段原样透传（折叠层零业务判别）', () => {
    const future = {
      ...record(5, 'completed'),
      status: 'futureStatus',
      futureColumn: '未来列',
      error: '第一行\n第二行😀',
    } as unknown as TurnSummary;

    const messages = foldSessionMessages([], [future]);
    const stored = messages[0]?.metadata?.record as unknown as Record<string, unknown>;

    expect(stored.status).toBe('futureStatus');
    expect(stored.futureColumn).toBe('未来列');
    expect(stored.error).toBe('第一行\n第二行😀');
  });

  it('turnId 两极（0 / -1 / 超大整数）→ 消息 id `turn-<id>` 稳定且互异', () => {
    const ids = [0, -1, Number.MAX_SAFE_INTEGER].map(
      (turnId) => foldSessionMessages([], [record(turnId, 'completed')])[0]?.id,
    );
    expect(ids).toEqual(['turn-0', 'turn--1', `turn-${Number.MAX_SAFE_INTEGER}`]);
    expect(new Set(ids).size).toBe(3);
  });

  it.each(['running', 'completed', 'failed', 'stopped'] as const)(
    '轮状态枚举 %s × 空 parts → metadata.record.status 逐字等于轮行（不做终态过滤）',
    (status) => {
      const row = record(21, status);
      const messages = foldSessionMessages([], [row]);

      expect(messages).toHaveLength(1);
      expect(messages[0]?.parts).toEqual([]);
      expect(messages[0]?.metadata?.record?.status).toBe(status);
      expect(messages[0]?.metadata?.record).toEqual(row);
    },
  );

  it('metadata 保真四类消息：assistant 取组首 seq、aux 取自身 seq、提问取提问 seq、parentToolUseId 只取 assistant 首事件', () => {
    const events: AgentEvent[] = [
      runStarted(0),
      userMessage(1, '问一句'),
      message(2, [{ kind: 'text', text: '答一句' }], 'tu_parent'),
      systemNotice(3),
      turnDone(4),
    ];

    const messages = fold(events);
    const byRole = new Map(messages.map((message) => [message.id, message.metadata]));

    // aux：逐事件自身 seq，归因恒 null
    expect(byRole.get('evt-0')).toEqual({ seq: 0, parentToolUseId: null });
    expect(byRole.get('evt-3')).toEqual({ seq: 3, parentToolUseId: null });
    expect(byRole.get('evt-4')).toEqual({ seq: 4, parentToolUseId: null });
    // 提问：提问事件 seq + 提问事件自身归因
    expect(byRole.get('turn-11-user')).toEqual({ seq: 1, parentToolUseId: null });
    // assistant：组首事件 seq（含 aux 首事件）+ 首个 assistant 密封事件归因 + 轮行
    expect(byRole.get('turn-11')).toEqual({
      seq: 0,
      parentToolUseId: 'tu_parent',
      record: record(TURN, 'completed'),
    });
  });

  it('超长与特殊字符在提问块内同样逐字保留（不截断不转义）', () => {
    const longText = '问'.repeat(1200);
    const specialText = '第一行\n第二行\u{0}结束😀🎉';
    // 每轮一个提问（同轮多提问只取首个——提问恰一条/轮）
    const messages = fold(
      [
        userMessage(0, longText, 11),
        message(1, [{ kind: 'text', text: '答一' }], null, 11),
        userMessage(2, specialText, 12),
        message(3, [{ kind: 'text', text: '答二' }], null, 12),
      ],
      [record(11, 'completed'), record(12, 'completed')],
    );

    const questions = messages.filter((message) => message.role === 'user');
    expect(questions.map((message) => message.id)).toEqual(['turn-11-user', 'turn-12-user']);
    expect(questions[0]?.parts).toEqual([{ type: 'text', text: longText, state: 'done' }]);
    expect(questions[1]?.parts).toEqual([{ type: 'text', text: specialText, state: 'done' }]);
  });
});

// ---------------------------------------------------------------------------
// foldSessionMessages：按轮分组（turnId 主路径 / 旧转录回退 / 零错位）
// ---------------------------------------------------------------------------

describe('foldSessionMessages：按轮分组', () => {
  it('三轮（正常 / 停止 / 正常）轮行逐一就位、零错位：折叠零 RunStarted 计数逻辑', () => {
    const turns = [record(11, 'completed'), record(12, 'stopped'), record(13, 'completed')];
    const events: AgentEvent[] = [
      runStarted(0, 11),
      userMessage(1, '第一问', 11),
      message(2, [{ kind: 'text', text: '第一答' }], null, 11),
      turnDone(3, { turnId: 11 }),
      // 停止轮：停止路径不发 TurnDone（环回直接返回），轮行由内核 converge 铸
      runStarted(4, 12),
      userMessage(5, '第二问', 12),
      runStarted(6, 13),
      userMessage(7, '第三问', 13),
      message(8, [{ kind: 'text', text: '第三答' }], null, 13),
      turnDone(9, { turnId: 13 }),
    ];

    const messages = foldSessionMessages(events, turns);
    const assistants = messages.filter((message) => message.role === 'assistant');

    expect(assistants.map((message) => message.id)).toEqual(['turn-11', 'turn-12', 'turn-13']);
    expect(assistants.map((message) => message.metadata?.record?.status)).toEqual([
      'completed',
      'stopped',
      'completed',
    ]);
    // 停止轮 parts 为空仍就位（空内容轮恒一条）
    expect(assistants[1]?.parts).toEqual([]);
    expect(messages.filter((message) => message.role === 'user').map((m) => m.id)).toEqual([
      'turn-11-user',
      'turn-12-user',
      'turn-13-user',
    ]);
  });

  it('旧转录（turnId 全 null）按 RunStarted 顺序分组：id 兜 evt-<seq>、每轮提问 + 一条 assistant', () => {
    const turns = [record(1, 'completed'), record(2, 'completed')];
    const events: AgentEvent[] = [
      runStarted(0, null),
      userMessage(1, '旧问一', null),
      message(2, [{ kind: 'text', text: '旧答一' }], null, null),
      runStarted(3, null),
      userMessage(4, '旧问二', null),
      message(5, [{ kind: 'text', text: '旧答二' }], null, null),
    ];

    const messages = foldSessionMessages(events, turns);
    const assistants = messages.filter((message) => message.role === 'assistant');

    expect(assistants.map((message) => message.id)).toEqual(['evt-2', 'evt-5']);
    expect(messages.filter((message) => message.role === 'user').map((m) => m.id)).toEqual([
      'evt-1',
      'evt-4',
    ]);
    expect(assistants.map((message) => message.metadata?.record?.turnId)).toEqual([1, 2]);
  });

  it('旧转录组内无 assistant 事件（残缺转录）时 id 不与同组 aux / 提问相撞', () => {
    const messages = foldSessionMessages([runStarted(0, null)], []);

    const ids = messages.map((message) => message.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(messages.at(-1)?.role).toBe('assistant');
  });

  it('TurnDone 不作轮边界：一轮内两枚 TurnDone 只产一条 assistant 且不新增轮', () => {
    const events = [message(1, [{ kind: 'text', text: '答' }]), turnDone(2), turnDone(3)];
    const messages = fold([...events], [record(TURN, 'completed')]);

    expect(messages.filter((message) => message.role === 'assistant')).toHaveLength(1);
    // TurnDone 仍逐事件保留在 aux 面（data-run-result 两层级不互相替代）
    expect(messages.filter((message) => message.parts[0]?.type === 'data-run-result')).toHaveLength(
      2,
    );
  });

  it('轮行缺席且组内有 TurnDone → 合成内存轮行（status 由 isError 派生、error 为 null）', () => {
    for (const isError of [false, true]) {
      const messages = foldSessionMessages([turnDone(1, { isError })], []);
      const stored = messages.at(-1)?.metadata?.record;

      expect(stored?.status).toBe(isError ? 'failed' : 'completed');
      expect(stored?.error).toBeNull();
      expect(stored?.finishedAt).toBe(TS);
      expect(stored?.numTurns).toBe(3);
    }
  });

  it('轮行缺席且组内无 TurnDone → record 为 null（不虚构 status、不抛错）', () => {
    const messages = foldSessionMessages([message(1, [])], []);

    expect(messages.at(-1)?.metadata?.record).toBeNull();
  });

  it('turns 为空 + 两组事件（turnId 11 / 12）→ 按组序追加两条 assistant，record 各为合成行 / null', () => {
    const events: AgentEvent[] = [
      message(1, [{ kind: 'text', text: '轮十一' }], null, 11),
      message(2, [{ kind: 'text', text: '轮十二' }], null, 12),
    ];

    const messages = foldSessionMessages(events, []);
    const assistants = messages.filter((message) => message.role === 'assistant');

    expect(assistants.map((message) => message.id)).toEqual(['turn-11', 'turn-12']);
    // 轮内无 TurnDone ⇒ 不虚构 status
    expect(assistants.map((message) => message.metadata?.record)).toEqual([null, null]);

    const withDone = foldSessionMessages([...events, turnDone(3, { turnId: 12 })], []);
    expect(withDone.filter((message) => message.role === 'assistant').map((m) => m.id)).toEqual([
      'turn-11',
      'turn-12',
    ]);
    expect(withDone.find((message) => message.id === 'turn-12')?.metadata?.record?.status).toBe(
      'completed',
    );
  });

  it.each([
    {
      label: '旧转录行数 == 组数（逐位配）',
      turns: [record(1, 'completed'), record(2, 'completed')],
      events: [runStarted(0, null), userMessage(1, '甲问', null), runStarted(2, null)],
      expectedAssistants: 2,
    },
    {
      label: '旧转录行数 > 组数（多余行各产空内容 assistant）',
      turns: [record(1, 'completed'), record(2, 'stopped'), record(3, 'failed')],
      events: [runStarted(0, null), userMessage(1, '甲问', null)],
      expectedAssistants: 3,
    },
    {
      label: '旧转录组数 > 行数（越界组走 synthesizeRow / null）',
      turns: [record(1, 'completed')],
      events: [
        runStarted(0, null),
        userMessage(1, '甲问', null),
        runStarted(2, null),
        userMessage(3, '乙问', null),
        turnDone(4, { turnId: null }),
      ],
      expectedAssistants: 2,
    },
  ])('旧转录组序 × 轮行数三形态：$label', ({ turns, events, expectedAssistants }) => {
    const messages = foldSessionMessages(events, turns);
    const assistants = messages.filter((message) => message.role === 'assistant');

    expect(assistants).toHaveLength(expectedAssistants);
    // 每轮 assistant 恒为末条，且组序不倒置
    expect(messages.at(-1)?.role).toBe('assistant');
    expect(new Set(messages.map((message) => message.id)).size).toBe(messages.length);
  });

  it('单元素极简：仅一枚 runStarted → 一条 aux system + 一条 parts 空 assistant', () => {
    const messages = foldSessionMessages([runStarted(0)], []);

    expect(messages.map((message) => message.role)).toEqual(['system', 'assistant']);
    expect(messages[0]?.parts.map((part) => part.type)).toEqual(['data-run-started']);
    expect(messages[1]?.parts).toEqual([]);
    expect(messages[1]?.metadata).toEqual({ seq: 0, parentToolUseId: null, record: null });
  });

  it('超大列表：1000 条密封事件多轮混合 → 输出保序、每轮 assistant 恰一条、seq 单调不减', () => {
    const events: AgentEvent[] = [];
    const turns: TurnSummary[] = [];
    for (let turnId = 0; turnId < 10; turnId += 1) {
      turns.push(record(turnId, 'completed'));
      for (let index = 0; index < 100; index += 1) {
        const seq = turnId * 100 + index;
        events.push(
          index === 0
            ? userMessage(seq, `第 ${turnId} 问`, turnId)
            : message(seq, [{ kind: 'text', text: `第 ${turnId}-${index} 段` }], null, turnId),
        );
      }
    }

    const messages = foldSessionMessages(events, turns);
    const assistants = messages.filter((message) => message.role === 'assistant');

    expect(assistants.map((message) => message.id)).toEqual(
      turns.map((row) => `turn-${row.turnId}`),
    );
    expect(assistants.map((message) => message.parts.length)).toEqual(turns.map(() => 99));
    const seqs = assistants.map((message) => message.metadata?.seq ?? -1);
    expect(seqs).toEqual([...seqs].sort((left, right) => left - right));
    // 提问与 assistant 交错成对（每轮一问一答，顺序不倒置）
    expect(messages.filter((message) => message.role === 'user')).toHaveLength(10);
  });

  it.each([
    { label: 'TurnDone 位于轮首', doneSeq: 1, deltaFirst: false },
    { label: 'TurnDone 位于轮尾', doneSeq: 2, deltaFirst: true },
  ])('$label：轮内单枚 TurnDone 只产一条 assistant、不新增轮', ({ doneSeq, deltaFirst }) => {
    const events: AgentEvent[] = deltaFirst
      ? [messageDelta(0, { kind: 'text', text: '增量' }), message(1, []), turnDone(doneSeq)]
      : [turnDone(doneSeq), message(2, [{ kind: 'text', text: '答' }])];

    const messages = fold(events, [record(TURN, 'completed')]);

    expect(messages.filter((message) => message.role === 'assistant')).toHaveLength(1);
    expect(messages.filter((message) => message.role === 'assistant')[0]?.id).toBe('turn-11');
  });
});

// ---------------------------------------------------------------------------
// Delta 防御跳过 + delta 路 chunk（id 由 step 内首个同类 delta 的 seq 派生）
// ---------------------------------------------------------------------------

describe('双层词汇：delta 防御跳过与 delta 路 chunk', () => {
  it('foldSessionMessages 防御跳过 messageDelta（delta 仅实时流可见，不进重放路径）', () => {
    const messages = fold([
      messageDelta(0, { kind: 'text', text: '增量' }),
      message(1, [{ kind: 'text', text: '密封' }]),
    ]);

    expect(messages.map((m) => m.id)).toEqual(['turn-11']);
  });

  it('eventToChunk 对 messageDelta 产出单条累积 delta chunk（part id 由该 delta 的 seq 派生）', () => {
    const textChunks = eventToChunk(messageDelta(2, { kind: 'text', text: '你' }));
    const thinkingChunks = eventToChunk(
      messageDelta(3, { kind: 'thinking', thinking: '想' }, 'tu_1'),
    );

    expect(textChunks).toEqual([{ type: 'text-delta', id: 't-2', delta: '你' }]);
    expect(thinkingChunks).toEqual([{ type: 'reasoning-delta', id: 'r-3', delta: '想' }]);
  });

  it('注入 deltaAnchor → 同 step 内多条 delta 复用首个同类 delta 的 seq（AC-4）', () => {
    const anchors = new Map<'text' | 'thinking', number>([['text', 5]]);
    const anchor = (kind: 'text' | 'thinking', seq: number) => anchors.get(kind) ?? seq;

    const first = eventToChunk(messageDelta(5, { kind: 'text', text: '甲' }), new Set(), anchor);
    const second = eventToChunk(messageDelta(9, { kind: 'text', text: '乙' }), new Set(), anchor);

    expect(first).toEqual([{ type: 'text-delta', id: 't-5', delta: '甲' }]);
    expect(second).toEqual([{ type: 'text-delta', id: 't-5', delta: '乙' }]);
  });

  it('纯 delta 序列（无任何密封事件）折叠为空：不炸、不产部件', () => {
    const messages = fold([
      messageDelta(0, { kind: 'text', text: '增量甲' }),
      messageDelta(1, { kind: 'thinking', thinking: '增量乙' }),
      messageDelta(2, { kind: 'text', text: '增量丙' }),
    ]);

    expect(messages.filter((message) => message.role === 'assistant')).toHaveLength(1);
    expect(assistantOf(messages)?.parts).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 两路同构：折叠路 vs 实时路（真实 useChat reducer 归约）
// ---------------------------------------------------------------------------

/** 同构投影：部件形状投影（`step-start` 与 data 部件不入对拍——前者是 reducer
 * 标记、后者是实时态 aux 载体，收口由折叠整表替换归一） */
function visibleShapeOf(message: AgentUIMessage | undefined): unknown {
  if (message === undefined) return undefined;
  return {
    id: message.id,
    parts: message.parts
      .filter((part) => part.type !== 'step-start')
      .filter((part) => !part.type.startsWith('data-'))
      .map((part) => {
        if (part.type === 'text' || part.type === 'reasoning') {
          return { type: part.type, text: part.text, state: part.state };
        }
        const tool = part as AgentToolPart;
        return {
          type: tool.type,
          toolCallId: tool.toolCallId,
          state: tool.state,
          input: tool.input,
          output: tool.state === 'output-available' ? tool.output : undefined,
        };
      }),
  };
}

/**
 * 实时路帧序的测试侧复刻（design「实时路状态机」）：`start` 每轮一次、
 * `start-step` 每响应一次、密封前恒 `reset-step`。权威实现与主判据在
 * `agent-transport.test.ts`；此处只用于「两路同构」的归约对照。
 */
function liveChunks(events: AgentEvent[], row: TurnSummary): AgentUIMessageChunk[] {
  const state = {
    started: false,
    startSeq: null as number | null,
    stepOpen: false,
    anchors: new Map<'text' | 'thinking', number>(),
    tools: new Set<string>(),
  };
  const chunks: AgentUIMessageChunk[] = [];
  const anchor = (kind: 'text' | 'thinking', seq: number) => state.anchors.get(kind) ?? seq;
  const ensureStart = (event: AgentEvent) => {
    if (state.started) return;
    state.started = true;
    state.startSeq = event.seq;
    chunks.push({
      type: 'start',
      messageId: event.turnId != null ? `turn-${event.turnId}` : `evt-${event.seq}`,
      messageMetadata: { seq: event.seq, parentToolUseId: null },
    });
  };
  for (const event of events) {
    ensureStart(event);
    if (event.kind === 'messageDelta') {
      const kind = event.delta.kind === 'text' ? 'text' : 'thinking';
      if (!state.stepOpen) {
        chunks.push({ type: 'start-step' });
        state.stepOpen = true;
        state.anchors.clear();
      }
      if (!state.anchors.has(kind)) {
        state.anchors.set(kind, event.seq);
        chunks.push(
          kind === 'text'
            ? { type: 'text-start', id: deltaPartId(event.seq, 'text') }
            : { type: 'reasoning-start', id: deltaPartId(event.seq, 'thinking') },
        );
      }
      chunks.push(...eventToChunk(event, state.tools, anchor));
      continue;
    }
    if (event.kind === 'message' && event.role === 'assistant') {
      if (!state.stepOpen) chunks.push({ type: 'start-step' });
      chunks.push({ type: 'reset-step' });
      state.stepOpen = false;
      state.anchors.clear();
      chunks.push(...eventToChunk(event, state.tools, anchor));
      for (const block of event.blocks) {
        if (block.kind === 'toolUse') state.tools.add(block.id);
      }
      continue;
    }
    chunks.push(...eventToChunk(event, state.tools, anchor));
  }
  chunks.push({
    type: 'finish',
    messageMetadata: { seq: state.startSeq, parentToolUseId: null, record: row },
  });
  return chunks;
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

/** 同批事件经实时路帧序流入 useChat reducer 归约后的消息序列（本地提问除外） */
async function reducedMessages(events: AgentEvent[]): Promise<AgentUIMessage[]> {
  const rendered = renderHook(() =>
    useChat<AgentUIMessage>({
      transport: replayTransport(liveChunks(events, record(TURN, 'completed'))),
    }),
  );
  await act(async () => {
    await rendered.result.current.sendMessage({
      id: 'local-q',
      parts: [{ type: 'text', text: '驱动发送' }],
    });
  });
  await waitFor(() => expect(rendered.result.current.status).toBe('ready'));
  return rendered.result.current.messages.filter((message) => message.id !== 'local-q');
}

/** 同 [`reducedMessages`]，但轮行可注入（停止轮 / 失败轮的收口形态） */
async function reducedMessagesWith(
  events: AgentEvent[],
  row: TurnSummary,
): Promise<AgentUIMessage[]> {
  const rendered = renderHook(() =>
    useChat<AgentUIMessage>({ transport: replayTransport(liveChunks(events, row)) }),
  );
  await act(async () => {
    await rendered.result.current.sendMessage({
      id: 'local-q',
      parts: [{ type: 'text', text: '驱动发送' }],
    });
  });
  await waitFor(() => expect(rendered.result.current.status).toBe('ready'));
  return rendered.result.current.messages.filter((message) => message.id !== 'local-q');
}

describe('两路同构：折叠路 vs 实时路（真实 reducer 归约）', () => {
  it('同一事件序列两路算出同一 assistant id（turn-<turnId>）且部件形状一致', async () => {
    const events = [
      runStarted(0),
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
    const folded = fold(events);

    expect(reduced.filter((m) => m.role === 'assistant')).toHaveLength(1);
    expect(reduced.find((m) => m.role === 'assistant')?.id).toBe('turn-11');
    expect(visibleShapeOf(assistantOf(reduced))).toEqual(visibleShapeOf(assistantOf(folded)));
    // record 归 metadata：两路同值
    expect(assistantOf(reduced)?.metadata?.record).toEqual(assistantOf(folded)?.metadata?.record);
  });

  it('一轮两次响应：A₁ 段密封块在 A₂ 段的 reset-step 后仍存活（前段不丢部件）', async () => {
    const events = [
      messageDelta(0, { kind: 'text', text: '第一' }),
      message(1, [{ kind: 'text', text: '第一段' }]),
      messageDelta(2, { kind: 'text', text: '第二' }),
      message(3, [{ kind: 'text', text: '第二段' }]),
    ];

    const reduced = await reducedMessages(events);
    const texts = assistantOf(reduced)
      ?.parts.filter((part) => part.type === 'text')
      .map((part) => part.type === 'text' && part.text);

    expect(texts).toEqual(['第一段', '第二段']);
    expect(reduced.filter((m) => m.role === 'assistant')).toHaveLength(1);
  });

  it('停止轮（无 TurnDone、轮行 stopped）：两路 assistant id 相等、部件形状一致、record 同为停止行', async () => {
    const events = [
      userMessage(0, '被停止的一轮'),
      messageDelta(1, { kind: 'text', text: '只写了一半' }),
      message(2, [{ kind: 'text', text: '半句回答' }]),
    ];
    const stoppedRow = record(TURN, 'stopped');
    const reduced = await reducedMessagesWith(events, stoppedRow);
    const folded = foldSessionMessages(events, [stoppedRow]);

    const reducedAssistant = assistantOf(reduced);
    const foldedAssistant = assistantOf(folded);
    expect(reducedAssistant?.id).toBe('turn-11');
    expect(reducedAssistant?.id).toBe(foldedAssistant?.id);
    expect(visibleShapeOf(reducedAssistant)).toEqual(visibleShapeOf(foldedAssistant));
    expect(reducedAssistant?.metadata?.record?.status).toBe('stopped');
    expect(foldedAssistant?.metadata?.record?.status).toBe('stopped');
  });
});

// ---------------------------------------------------------------------------
// eventToChunk：增量边界（只产块 chunk，帧归 transport）
// ---------------------------------------------------------------------------

describe('eventToChunk：增量边界', () => {
  it('密封 assistant 事件不产任何帧 chunk（start / start-step / reset-step / finish 归 transport）', () => {
    const chunks = eventToChunk(message(5, [{ kind: 'text', text: '答' }]));
    const frameTypes = ['start', 'start-step', 'reset-step', 'finish'];

    expect(chunks.map((chunk) => chunk.type)).toEqual(['text-start', 'text-delta', 'text-end']);
    for (const chunk of chunks) expect(frameTypes).not.toContain(chunk.type);
  });

  it('role = user 的密封事件零 chunk（chunk 协议无 role 字段，提问由会话状态侧承载）', () => {
    expect(eventToChunk(userMessage(3, '继续'))).toEqual([]);
    expect(eventToChunk({ ...userMessage(4, ''), blocks: [] } as AgentEvent)).toEqual([]);
  });

  it('role = tool 且同 id 已在视野内 → 只投 tool-output-available（无 input 重投）', () => {
    const chunks = eventToChunk(
      toolMessage(2, [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }]),
      new Set(['tu_1']),
    );

    expect(chunks).toEqual([
      {
        type: 'tool-output-available',
        toolCallId: 'tu_1',
        output: { content: '目录内容', isError: false },
      },
    ]);
  });

  it('实时到达的无主 toolResult 产出就地合成占位部件的 chunk 组，与折叠路径语义一致', () => {
    const chunks = eventToChunk(
      message(0, [{ kind: 'toolResult', id: 'orphan_1', content: '无主结果', isError: false }]),
    );

    expect(chunks).toEqual([
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

    expect(chunks).toEqual([
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

  it('空 blocks 事件产出空 chunk 组（零块 chunk），不抛错', () => {
    expect(eventToChunk(message(0, []))).toEqual([]);
  });

  it('raw 事件产出单条 data-raw chunk，不抛错', () => {
    expect(eventToChunk(raw(1))).toEqual([
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

  it('delta 载荷与 seq 边界：空串 / 超长 / 特殊字符逐字透传，id 不夹取', () => {
    const long = '超'.repeat(1200);
    expect(eventToChunk(messageDelta(0, { kind: 'text', text: '' }))).toEqual([
      { type: 'text-delta', id: 't-0', delta: '' },
    ]);
    expect(
      eventToChunk(messageDelta(Number.MAX_SAFE_INTEGER, { kind: 'text', text: long })),
    ).toEqual([{ type: 'text-delta', id: `t-${Number.MAX_SAFE_INTEGER}`, delta: long }]);
    expect(eventToChunk(messageDelta(1, { kind: 'thinking', thinking: 'a\n\0🎉' }))).toEqual([
      { type: 'reasoning-delta', id: 'r-1', delta: 'a\n\0🎉' },
    ]);
  });
});

// ---------------------------------------------------------------------------
// eventToChunk：实时路跨消息配对视野（currentMessageToolCalls）
// ---------------------------------------------------------------------------

describe('eventToChunk：实时路跨消息配对视野', () => {
  const resultEvent = (seq: number, id: string, content: string): AgentEvent =>
    toolMessage(seq, [{ kind: 'toolResult', id, content, isError: false }]);

  it('命中视野的纯结果事件只投 tool-output-available：不重投 input（与 toolUse 合住一条消息）', () => {
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
        'tool-input-available',
        'tool-output-available',
      ]);
    }
    expect(eventToChunk(resultEvent(2, 'tu_1', '目录内容'))).toEqual(
      eventToChunk(resultEvent(2, 'tu_1', '目录内容'), new Set()),
    );
  });

  it('同事件混含命中与未命中结果：命中项先就地补全，其余块照常起块', () => {
    const chunks = eventToChunk(
      message(2, [
        { kind: 'toolResult', id: 'tu_1', content: '命中结果', isError: false },
        { kind: 'toolResult', id: 'tu_9', content: '未命中结果', isError: true },
      ]),
      new Set(['tu_1']),
    );

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      'tool-output-available',
      'tool-input-available',
      'tool-output-available',
    ]);
    expect(chunks[0]).toEqual({
      type: 'tool-output-available',
      toolCallId: 'tu_1',
      output: { content: '命中结果', isError: false },
    });
    expect(chunks[1]).toEqual({
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
      'tool-input-available',
      'tool-output-available',
    ]);
  });
});

// ---------------------------------------------------------------------------
// deltaPartId 直调：签名 / 稳定性 / 互异 / 与整块部件 id 无碰撞域
// ---------------------------------------------------------------------------

describe('deltaPartId 直调：稳定性与互异契约', () => {
  it('同 seq 同类幂等；text / thinking 两族互异；不同 seq 互异', () => {
    expect(deltaPartId(7, 'text')).toBe('t-7');
    expect(deltaPartId(7, 'thinking')).toBe('r-7');
    expect(deltaPartId(7, 'text')).toBe(deltaPartId(7, 'text'));
    expect(deltaPartId(7, 'text')).not.toBe(deltaPartId(7, 'thinking'));
    expect(deltaPartId(7, 'text')).not.toBe(deltaPartId(8, 'text'));
    expect(deltaPartId(7, 'thinking')).not.toBe(deltaPartId(8, 'thinking'));
  });

  it('旧常量语义已退役：不以 t-delta- / r-delta- 开头，且与整块部件 id t-<seq>-<n> 形状不相等', () => {
    for (const id of [deltaPartId(7, 'text'), deltaPartId(7, 'thinking')]) {
      expect(id.startsWith('t-delta-')).toBe(false);
      expect(id.startsWith('r-delta-')).toBe(false);
    }
    expect(deltaPartId(7, 'text')).not.toBe('t-7-0');
    expect(deltaPartId(7, 'thinking')).not.toBe('r-7-0');
  });

  it('seq 边界（0 / -1 / 超大整数 / 越签名脏值）稳定输出不夹取', () => {
    expect(deltaPartId(0, 'text')).toBe('t-0');
    expect(deltaPartId(-1, 'text')).toBe('t--1');
    expect(deltaPartId(Number.MAX_SAFE_INTEGER, 'thinking')).toBe(`r-${Number.MAX_SAFE_INTEGER}`);
    expect(deltaPartId(undefined as unknown as number, 'text')).toBe('t-undefined');
  });

  it('part 枚举两值 + 非法值（reasoning / 空串）→ 非 text 分支一律 `r-<seq>`，不抛错', () => {
    expect(deltaPartId(4, 'text')).toBe('t-4');
    expect(deltaPartId(4, 'thinking')).toBe('r-4');
    for (const illegal of ['reasoning', ''] as unknown as ('text' | 'thinking')[]) {
      expect(() => deltaPartId(4, illegal)).not.toThrow();
      expect(deltaPartId(4, illegal)).toBe('r-4');
    }
  });
});

// ---------------------------------------------------------------------------
// DeltaPartAnchor（类型面）：`eventToChunk` 第三参契约
// ---------------------------------------------------------------------------

describe('DeltaPartAnchor：类型契约与缺省行为', () => {
  it('第三参接受 (kind, seq) => number 回调；缺省不传时逐 delta 用自身 seq', () => {
    expectTypeOf<DeltaPartAnchor>().toEqualTypeOf<
      (kind: 'text' | 'thinking', seq: number) => number
    >();
    expectTypeOf<Parameters<typeof eventToChunk>[2]>().toEqualTypeOf<DeltaPartAnchor | undefined>();

    const anchor: DeltaPartAnchor = (kind, seq) => (kind === 'text' ? seq + 100 : seq + 200);
    expect(eventToChunk(messageDelta(1, { kind: 'text', text: '甲' }), new Set(), anchor)).toEqual([
      { type: 'text-delta', id: 't-101', delta: '甲' },
    ]);
    // 缺省实现 = 逐 delta 独立 part（id 取自身 seq）
    expect(eventToChunk(messageDelta(1, { kind: 'thinking', thinking: '乙' }))).toEqual([
      { type: 'reasoning-delta', id: 'r-1', delta: '乙' },
    ]);
  });
});

// ---------------------------------------------------------------------------
// IR 三层（AC-7）：计划层对拍 / 视野差异快照 / 混合配对 / 规模与字符边界 / 私有性
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

describe('计划层对拍：折叠路 vs 实时路 reducer（同消息场景两路同形）', () => {
  it.each(Object.entries(SAME_MESSAGE_CASES))('%s：两路部件形状相等', async (_label, blocks) => {
    const events = [message(7, blocks)];
    const reduced = await reducedMessages(events);

    expect(reduced.filter((m) => m.role === 'assistant').map((m) => m.id)).toEqual(['turn-11']);
    expect(visibleShapeOf(assistantOf(reduced))).toEqual(visibleShapeOf(assistantOf(fold(events))));
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

    const folded = fold(events);
    expect(assistantOf(folded)?.parts).toHaveLength(1);
    expect(assistantOf(folded)?.parts[0]).toMatchObject({
      type: 'tool-Read',
      toolCallId: 'tu_9',
      state: 'output-available',
      input: { path: 'a.ts' },
      output: { content: '文件内容', isError: true },
    });

    const reduced = await reducedMessages(events);
    expect(visibleShapeOf(assistantOf(reduced))).toEqual(visibleShapeOf(assistantOf(folded)));
  });
});

describe('视野快照：跨消息差异锁定（MUST NOT 用单一路径 fixture 掩饰）', () => {
  const crossMessage = (): AgentEvent[] => [
    message(0, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
    toolMessage(1, [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }]),
  ];

  it('折叠路（global 视野）：跨消息配对收敛为单卡 output-available，两事件同住一条消息', () => {
    const folded = fold(crossMessage());

    expect(folded).toHaveLength(1);
    expect(folded[0]?.parts).toHaveLength(1);
    expect(folded[0]?.parts[0]).toMatchObject({
      type: 'tool-Bash',
      toolCallId: 'tu_1',
      state: 'output-available',
      output: { content: '目录内容', isError: false },
    });
  });

  it('纯函数 local 视野（未携流内视野）：use 侧 input-available + 孤儿 result 占位卡两件', () => {
    const [useEvent, resultEvent] = crossMessage() as [AgentEvent, AgentEvent];
    const useChunks = eventToChunk(useEvent);
    const orphanChunks = eventToChunk(resultEvent);

    expect(useChunks).toEqual([
      {
        type: 'tool-input-available',
        toolCallId: 'tu_1',
        toolName: 'Bash',
        input: { command: 'ls' },
      },
    ]);
    expect(orphanChunks).toEqual([
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

  it('携流内视野（currentMessageToolCalls 本轮累积）→ 结果就地补全、终态与折叠路同形', async () => {
    const events = crossMessage();
    const reduced = await reducedMessages(events);

    // currentToolCalls 跨 step 不清：结果事件只投 tool-output-available（不开新 step、不产双卡）
    expect(visibleShapeOf(assistantOf(reduced))).toEqual(visibleShapeOf(assistantOf(fold(events))));
    expect(reduced.filter((m) => m.role === 'assistant')).toHaveLength(1);
  });
});

describe('foldSessionMessages：多 toolUse / result 混合配对', () => {
  it('同 id 重复 result 取首个；result 在 use 之前的同消息仍配对收敛单卡', () => {
    const events = [
      message(0, [
        { kind: 'toolResult', id: 'tu_a', content: '首个结果', isError: false },
        { kind: 'toolUse', id: 'tu_a', name: 'Bash', input: { command: 'pwd' } },
        { kind: 'toolUse', id: 'tu_b', name: 'Read', input: { path: 'b.ts' } },
      ]),
      toolMessage(1, [
        { kind: 'toolResult', id: 'tu_a', content: '重复结果被丢弃', isError: true },
        { kind: 'toolResult', id: 'tu_b', content: 'b 的结果', isError: false },
      ]),
    ];

    const parts = assistantOf(fold(events))?.parts ?? [];
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
  });
});

describe('foldSessionMessages：规模与字符边界', () => {
  it('1000 条密封事件（同轮）不抛错、部件保序且 metadata.seq 取组首事件', () => {
    const events: AgentEvent[] = [];
    for (let seq = 0; seq < 1000; seq += 1) {
      events.push(message(seq, [{ kind: 'text', text: `第 ${seq} 段` }]));
    }

    const messages = fold(events);
    const assistant = assistantOf(messages);

    expect(messages).toHaveLength(1);
    expect(assistant?.parts).toHaveLength(1000);
    expect(assistant?.parts[0]).toMatchObject({ type: 'text', text: '第 0 段' });
    expect(assistant?.parts[999]).toMatchObject({ type: 'text', text: '第 999 段' });
    expect(assistant?.metadata?.seq).toBe(0);
  });

  it('超长文本（> 1000 chars）与特殊字符（换行 / NUL / emoji）逐字保留', () => {
    const longText = '超'.repeat(1200);
    const specialText = '第一行\n第二行\u{0}结束😀🎉';
    const messages = fold([
      message(0, [{ kind: 'text', text: longText }]),
      message(1, [{ kind: 'text', text: specialText }]),
    ]);
    const parts = assistantOf(messages)?.parts ?? [];

    expect(parts[0]).toMatchObject({ type: 'text', text: longText });
    expect(parts[1]).toMatchObject({ type: 'text', text: specialText });
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
      'reasoning-start',
      'reasoning-delta',
      'reasoning-end',
      'text-start',
      'text-delta',
      'text-end',
      'tool-input-available',
      'tool-output-available',
    ]);
    expect(chunks[0]).toMatchObject({ type: 'reasoning-start', id: 'r-5-0' });
    expect(chunks[3]).toMatchObject({ type: 'text-start', id: 't-5-1' });
    expect(chunks[6]).toMatchObject({
      type: 'tool-input-available',
      toolCallId: 'tu_x',
      toolName: 'Glob',
      input: { pattern: '*.ts' },
    });
    expect(chunks[7]).toMatchObject({
      type: 'tool-output-available',
      toolCallId: 'tu_x',
      output: { content: '命中 2 个', isError: false },
    });
  });

  it('aux 路：单 data 部件 chunk（载荷与折叠路同事件逐字段相等）', () => {
    const event = turnDone(9);
    const chunks = eventToChunk(event);
    const folded = fold([event]);

    expect(chunks.map((chunk) => chunk.type)).toEqual(['data-run-result']);
    expect(chunks[0]).toEqual(folded[0]?.parts[0]);
  });

  it('未知 aux 事件（未来变体）→ 单条 data-raw，chunk 流不中断', () => {
    const future = {
      seq: 3,
      turnId: TURN,
      timestampMs: TS,
      kind: 'futureVariant',
      note: '未来字段',
    } as unknown as AgentEvent;

    const chunks = eventToChunk(future);

    expect(chunks.map((chunk) => chunk.type)).toEqual(['data-raw']);
    expect(chunks[0]).toMatchObject({
      type: 'data-raw',
      data: { seq: 3, eventType: 'unknown' },
    });
  });

  it.each([
    ['runStarted', (): AgentEvent => runStarted(2)],
    ['systemNotice', (): AgentEvent => systemNotice(3)],
    ['turnDone', (): AgentEvent => turnDone(4)],
    ['raw', (): AgentEvent => raw(5)],
  ])('aux 四变体 %s：单条 data 部件 chunk，载荷与折叠路同事件逐字段相等', (_label, build) => {
    const event = build();
    const chunks = eventToChunk(event);
    const folded = fold([event]);

    expect(chunks).toHaveLength(1);
    expect(chunks[0]?.type.startsWith('data-')).toBe(true);
    // 帧 chunk 一个都不产（帧归 transport）
    expect(chunks.map((chunk) => chunk.type)).not.toContain('start');
    expect(chunks[0]).toEqual(folded[0]?.parts[0]);
  });

  it('全部块均命中视野（纯结果事件）→ 返回的 chunk 全是 tool-output-available', () => {
    const chunks = eventToChunk(
      toolMessage(2, [
        { kind: 'toolResult', id: 'tu_1', content: '甲', isError: false },
        { kind: 'toolResult', id: 'tu_2', content: '乙', isError: true },
      ]),
      new Set(['tu_1', 'tu_2']),
    );

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      'tool-output-available',
      'tool-output-available',
    ]);
    expect(
      chunks.some((chunk) => chunk.type.startsWith('text-') || chunk.type.startsWith('reasoning-')),
    ).toBe(false);
  });

  it('未知块变体（未来新增）→ 不产部件、不抛错、不产 data-raw 兜底', () => {
    const unknownBlock = { kind: 'futureBlock', text: '未知' } as unknown as AgentBlock;

    expect(eventToChunk(message(1, [unknownBlock]))).toEqual([]);
  });
});

describe('IR 三层私有性（D11）：公共面恰三出口', () => {
  it('模块运行时导出键集合锁定（BlockPlan / planBlock / plansToParts / plansToChunks / PairingScope 不外露）', () => {
    expect(Object.keys(adapterModule).sort()).toEqual([
      'deltaPartId',
      'eventToChunk',
      'foldSessionMessages',
    ]);
    for (const internal of [
      'BlockPlan',
      'planBlock',
      'plansToParts',
      'plansToChunks',
      'PairingScope',
      'auxEventData',
      'eventsToUIMessages',
      'provisionalMessageId',
      'runRecordToUIMessage',
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
    const messages = fold([message(1, [searchUseBlock('srvtoolu_1', '今日汇率')])]);

    const part = assistantOf(messages)?.parts[0];
    expect(part).toEqual({
      type: 'tool-web_search',
      toolCallId: 'srvtoolu_1',
      state: 'input-available',
      input: { query: '今日汇率' },
    });
    expect(part !== undefined && 'output' in part).toBe(false);
    // AgentDataParts 成员零新增：不产任何 data-* 部件
    expect(assistantOf(messages)?.parts.every((part) => !part.type.startsWith('data-'))).toBe(true);
  });

  it('use 块自带 rawResult（pause_turn 未拆分）→ output-available 且 output 为加位形态', () => {
    const results = [searchSource('https://a/1', '甲'), searchSource('https://b/2')];
    const messages = fold([
      message(1, [searchUseBlock('srvtoolu_2', '查询词', { type: 'x' }, results)]),
    ]);

    const part = assistantOf(messages)?.parts[0];
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
    const messages = fold([
      message(1, [searchUseBlock('srvtoolu_err', '超限')]),
      message(2, [searchResultBlock('srvtoolu_err', [], 'max_uses_exceeded')]),
    ]);

    const part = assistantOf(messages)?.parts[0];
    expect(part?.type === 'tool-web_search' && part.output).toEqual({
      content: '',
      isError: true,
      search: { results: [], error: 'max_uses_exceeded' },
    });
  });
});

describe('agent-adapter：web_search 跨消息配对与孤儿 result 侧', () => {
  it('消息 A 携 use、消息 B 携同 id result → 该 id 收敛为单个 tool 部件（result 侧不重复呈现）', () => {
    const messages = fold([
      message(1, [searchUseBlock('srvtoolu_x', '跨消息查询')]),
      message(2, [searchResultBlock('srvtoolu_x', [searchSource('https://x/1', '跨消息来源')])]),
    ]);

    // 两段响应累积在同一轮同一条 assistant 消息内（pause_turn 不产第二条气泡）
    expect(messages).toHaveLength(1);
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
  });

  it('孤儿 result 侧（无配对 use）→ input 为 null、output.search 携来源、toolCallId 取块 id', () => {
    const messages = fold([
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

  it('视野差异显式锁定：折叠路单卡 output-available；纯函数 local 视野 input-available + 孤儿 output-available', () => {
    const events = [
      message(1, [searchUseBlock('srvtoolu_view', '视野查询')]),
      message(2, [searchResultBlock('srvtoolu_view', [searchSource('https://v/1')])]),
    ];

    // 折叠路（global 视野）：单条消息 + 单卡
    const folded = fold(events);
    expect(folded).toHaveLength(1);
    expect(folded[0]?.parts).toHaveLength(1);
    expect(folded[0]?.parts[0]?.type === 'tool-web_search' && folded[0]?.parts[0]?.state).toBe(
      'output-available',
    );

    // 纯函数 local 视野（未携流内视野）：两事件各自起块
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

    // 同消息配对时两入口产物一致
    const sameMessage = [message(4, [searchUseBlock('srvtoolu_same', '同消息查询')])];
    expect(
      eventToChunk(sameMessage[0] as AgentEvent).filter(
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
    expect(assistantOf(fold(sameMessage))?.parts).toHaveLength(1);
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
    const messages = fold([message(1, [{ kind: 'text', text: '带引用正文', citations }])]);

    const part = assistantOf(messages)?.parts[0];
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
      const part = fold([message(1, [block])])[0]?.parts[0];
      expect(part?.type === 'text' && part.providerMetadata).toBeUndefined();
    }
  });

  it('chunk 路：text-start 与 text-delta 带同一份 metadata，text-end 不带；经 reducer 归约后与折叠路同形', async () => {
    const citations = [citation('https://cite/1', '引用标题')];
    const events: AgentEvent[] = [message(1, [{ kind: 'text', text: 'chunk 正文', citations }])];

    const chunks = eventToChunk(events[0] as AgentEvent);
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

    // 折叠路同形（两路同取该轮 assistant 的首个 text 部件——实时路的
    // `step-start` 标记部件是 reducer 内部产物，不参与呈现与对拍）
    const textOf = (messages: AgentUIMessage[]) =>
      assistantOf(messages)?.parts.find((part) => part.type === 'text');
    const foldedText = textOf(fold(events));
    const reducedText = textOf(await reducedMessages(events));
    expect(reducedText?.type === 'text' && reducedText.providerMetadata).toEqual(
      foldedText?.type === 'text' ? foldedText.providerMetadata : undefined,
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
      const messages = fold([
        message(1, [searchUseBlock('srvtoolu_edge', query, { type: 'x' }, results)]),
      ]);
      const part = assistantOf(messages)?.parts[0] as unknown as AgentToolPart;
      expect(part.type).toBe('tool-web_search');
      expect(part.input).toEqual({ query });
      expect(part.state).toBe('output-available');
      expect(part.state === 'output-available' ? part.output.search?.results.length : -1).toBe(
        results.length,
      );
    }
  });

  it('rawUse 为空对象（非 null）仍判 use 侧：查询词取自块 query 字段而非 rawUse', () => {
    const messages = fold([
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
    expect(fold([message(1, [defaults])])[0]?.parts).toEqual([
      {
        type: 'tool-web_search',
        toolCallId: 'srvtoolu_defaults',
        state: 'output-available',
        input: null,
        output: { content: '', isError: false, search: { results: [], error: null } },
      },
    ]);

    // 有配对 → 被 use 消费、返回 null（不重复呈现）
    const paired = fold([
      message(2, [searchUseBlock('srvtoolu_defaults', '配对查询')]),
      message(3, [defaults]),
    ]);
    expect(paired).toHaveLength(1);
    expect(paired[0]?.parts).toHaveLength(1);
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

    const messages = fold(events);
    const assistantParts = assistantOf(messages)?.parts ?? [];

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
    const partTypes = messages.flatMap((message) => message.parts.map((part) => part.type));
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

// ---------------------------------------------------------------------------
// steer-running-agent-step：按轮产出的「补充说明」半边（AC-10）
//
// 每轮 user 消息由「提问（组内首条）」细化为「提问 + 补充说明（组内其余各一条，
// id 锚在自身 seq，序在提问之后、assistant 之前）」。`entryToMessages` 为私有件，
// 行为统一经公共入口 `foldSessionMessages` 观测。
// ---------------------------------------------------------------------------

/** 该轮 user 消息（提问 + 补充说明，按产出序） */
function userMessagesOf(messages: AgentUIMessage[]): AgentUIMessage[] {
  return messages.filter((message) => message.role === 'user');
}

describe('foldSessionMessages：轮中段补充说明（AC-10）', () => {
  it('轮中段呈现：提问 → 补充说明 → assistant 三序，aux 逐事件独立不变', () => {
    const messages = fold([
      userMessage(3, '首轮提问'),
      message(5, [{ kind: 'text', text: '首轮结论' }]),
      userMessage(9, '补充说明'),
      message(11, [{ kind: 'text', text: '补充后结论' }]),
      turnDone(13),
    ]);

    expect(messages.filter((m) => m.role !== 'system').map((m) => m.id)).toEqual([
      'turn-11-user',
      'evt-9',
      'turn-11',
    ]);
    // aux 逐事件独立（turnDone 一条 system 消息）
    expect(messages.filter((m) => m.role === 'system')).toHaveLength(1);
    // assistant 恒为该轮末条
    expect(messages.at(-1)?.id).toBe('turn-11');
  });

  it('补充说明 parts 与提问同一表达式产出：text 块直映射（文本逐字、state done）', () => {
    const messages = fold([
      userMessage(3, '提问正文'),
      userMessage(9, '补充正文'),
      message(11, [{ kind: 'text', text: '结论' }]),
    ]);

    const users = userMessagesOf(messages);
    expect(users.map((m) => m.parts)).toEqual([
      [{ type: 'text', text: '提问正文', state: 'done' }],
      [{ type: 'text', text: '补充正文', state: 'done' }],
    ]);
  });

  it('id 族不撞：一条提问 + 两条补充说明 → 三条 user id 两两不等，assistant 恰一条', () => {
    const messages = fold([
      userMessage(3, '提问'),
      userMessage(9, '补充一'),
      userMessage(11, '补充二'),
      message(13, [{ kind: 'text', text: '结论' }]),
    ]);

    const users = userMessagesOf(messages);
    expect(users.map((m) => m.id)).toEqual(['turn-11-user', 'evt-9', 'evt-11']);
    expect(new Set(users.map((m) => m.id)).size).toBe(users.length);
    expect(messages.filter((m) => m.role === 'assistant')).toHaveLength(1);
  });

  it('补充说明不入 assistant（反向判据）：assistant parts 不含补充说明文本且恒为末条', () => {
    const messages = fold([
      userMessage(3, '提问'),
      userMessage(9, '不该进 assistant 的补充'),
      message(11, [{ kind: 'text', text: '结论' }]),
    ]);

    const assistant = assistantOf(messages);
    const assistantText = (assistant?.parts ?? [])
      .filter((part) => part.type === 'text')
      .map((part) => (part as { text: string }).text);
    expect(assistantText).not.toContain('不该进 assistant 的补充');
    expect(messages.at(-1)?.role).toBe('assistant');
  });

  it('重放不丢：同一转录连续折叠两次 → id 序列与消息序逐字一致', () => {
    const events = [
      userMessage(3, '提问'),
      message(5, [{ kind: 'text', text: '结论一' }]),
      userMessage(9, '补充'),
      message(11, [{ kind: 'text', text: '结论二' }]),
      turnDone(13),
    ];
    const first = fold(events).map((m) => m.id);
    const second = fold(events).map((m) => m.id);
    expect(second).toEqual(first);
  });

  it('旧转录（turnId 全 null）：组内首条为提问、其余为补充说明，全部两两不等', () => {
    const messages = fold([
      runStarted(0, null),
      userMessage(1, '旧提问', null),
      message(2, [{ kind: 'text', text: '旧结论' }], null, null),
      userMessage(3, '旧补充', null),
      turnDone(4),
    ]);

    const ids = messages.map((m) => m.id);
    expect(new Set(ids).size).toBe(ids.length);
    const users = userMessagesOf(messages);
    expect(users.map((m) => m.id)).toEqual(['evt-1', 'evt-3']);
    expect(messages.at(-1)?.role).toBe('assistant');
    expect(messages.at(-1)?.id).not.toBe('evt-3');
  });

  it('零 / 单 user 形态：单条只有提问、零条零 user（既有形状零变）', () => {
    const single = fold([userMessage(3, '唯一提问'), message(5, [{ kind: 'text', text: '结论' }])]);
    expect(userMessagesOf(single).map((m) => m.id)).toEqual(['turn-11-user']);

    const none = fold([message(5, [{ kind: 'text', text: '结论' }])]);
    expect(userMessagesOf(none)).toHaveLength(0);
    expect(assistantOf(none)).toBeDefined();
  });

  it('正文边界：空串 / 换行 / NUL 与 emoji / 超长逐字保留（不截断、不转义）', () => {
    const texts = ['', 'a\nb', 'a\0b🚀', '字'.repeat(1200)];
    for (const text of texts) {
      const messages = fold([
        userMessage(3, '提问'),
        userMessage(9, text),
        message(11, [{ kind: 'text', text: '结论' }]),
      ]);
      expect(userMessagesOf(messages)[1]?.parts).toEqual([{ type: 'text', text, state: 'done' }]);
    }
  });

  it('多轮隔离：两轮各一条补充说明 → 各落本轮提问与 assistant 之间', () => {
    const messages = fold(
      [
        userMessage(1, '第一问', 11),
        message(2, [{ kind: 'text', text: '第一答' }], null, 11),
        userMessage(3, '第一补充', 11),
        turnDone(4, { turnId: 11 }),
        userMessage(5, '第二问', 12),
        message(6, [{ kind: 'text', text: '第二答' }], null, 12),
        userMessage(7, '第二补充', 12),
        turnDone(8, { turnId: 12 }),
      ],
      [record(11, 'completed'), record(12, 'completed')],
    );

    const ids = messages.map((m) => m.id);
    const firstQuestion = ids.indexOf('turn-11-user');
    const firstSupplement = ids.indexOf('evt-3');
    const firstAssistant = ids.indexOf('turn-11');
    const secondQuestion = ids.indexOf('turn-12-user');
    const secondSupplement = ids.indexOf('evt-7');
    const secondAssistant = ids.indexOf('turn-12');
    for (const [name, value] of [
      ['第一问', firstQuestion],
      ['第一补充', firstSupplement],
      ['第一答', firstAssistant],
      ['第二问', secondQuestion],
      ['第二补充', secondSupplement],
      ['第二答', secondAssistant],
    ] as [string, number][]) {
      expect(value, `${name} 在场`).toBeGreaterThanOrEqual(0);
    }
    expect(firstQuestion).toBeLessThan(firstSupplement);
    expect(firstSupplement).toBeLessThan(firstAssistant);
    expect(firstAssistant).toBeLessThan(secondQuestion);
    expect(secondQuestion).toBeLessThan(secondSupplement);
    expect(secondSupplement).toBeLessThan(secondAssistant);
  });

  it('插入锚点（DD6）：补充说明事件与 assistant 块交错 → 仍排在提问之后、assistant 之前', () => {
    const messages = fold([
      message(1, [{ kind: 'text', text: '先说的话' }]),
      userMessage(3, '提问'),
      message(5, [{ kind: 'text', text: '中间结论' }]),
      userMessage(9, '补充'),
      message(11, [{ kind: 'text', text: '末结论' }]),
    ]);

    const ids = messages.map((m) => m.id);
    expect(ids.indexOf('turn-11-user')).toBeLessThan(ids.indexOf('evt-9'));
    expect(ids.indexOf('evt-9')).toBeLessThan(ids.indexOf('turn-11'));
    expect(ids.at(-1)).toBe('turn-11');
  });
});
