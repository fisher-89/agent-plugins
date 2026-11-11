import { describe, expect, it } from 'vite-plus/test';

import type { AgentBlock, AgentEvent, TurnSummary } from '../types/dto';
import { foldSessionMessages, type AgentUIMessage } from './agent-adapter';
import {
  conversationIdentity,
  filterConversation,
  hasVisibleParts,
  userTurnSpans,
} from './agent-virtual';

/**
 * 对话透镜的呈现层纯函数（零 mock、零 jsdom 依赖）：可见部件判据、过滤判据单点、
 * 吸附层几何。fixture 优先由适配层真实折叠产出（`parts` / `metadata.record` 的
 * 形状是被测判据的输入本体）；只有越过类型签名的脏值才用字面量构造。
 */

const TS = 1727000000000;

type Part = AgentUIMessage['parts'][number];

function messageEvent(seq: number, blocks: AgentBlock[], turnId = seq + 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks,
    parentToolUseId: null,
  };
}

function userEvent(seq: number, text: string, turnId = seq + 1): AgentEvent {
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

function runStartedEvent(seq: number, turnId = seq + 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function systemNoticeEvent(seq: number, turnId = seq + 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'systemNotice',
    subtype: 'permission_denial',
    payload: {},
  };
}

function rawEvent(seq: number, turnId = seq + 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery"}',
  };
}

function recordRow(
  status: TurnSummary['status'],
  turnId: number,
  error: string | null = null,
): TurnSummary {
  return {
    turnId,
    sessionId: 'ses-0-1727000000000',
    status,
    startedAt: TS,
    finishedAt: TS + 999,
    numTurns: 2,
    costUsd: 0.2,
    durationMs: 800,
    error,
  };
}

/** 折叠唯一入口的薄包装（本套件只关心呈现层判据，轮行按需给） */
function fold(events: AgentEvent[], turns: TurnSummary[] = []): AgentUIMessage[] {
  return foldSessionMessages(events, turns);
}

function textPart(text: string): Part {
  return { type: 'text', text, state: 'done' };
}

function reasoningPart(text: string): Part {
  return { type: 'reasoning', text, state: 'done' };
}

function toolPart(name: string): Part {
  return {
    type: `tool-${name}`,
    toolCallId: 'tu_1',
    state: 'input-available',
    input: { ls: true },
  };
}

/** 越过联合类型的脏部件（判据不得枚举黑名单） */
function dirtyPart(type: string): Part {
  return { type } as Part;
}

function assistantWith(
  parts: Part[],
  id = 'assistant-1',
  status: TurnSummary['status'] | null = null,
): AgentUIMessage {
  return {
    id,
    role: 'assistant',
    metadata: {
      seq: null,
      parentToolUseId: null,
      record: status === null ? null : recordRow(status, 1),
    },
    parts,
  };
}

function userWith(text: string, id = 'user-1'): AgentUIMessage {
  return {
    id,
    role: 'user',
    metadata: { seq: null, parentToolUseId: null, record: null },
    parts: [textPart(text)],
  };
}

function systemWith(id: string): AgentUIMessage {
  return {
    id,
    role: 'system',
    metadata: { seq: 0, parentToolUseId: null },
    parts: [
      {
        type: 'data-system-notice',
        data: { seq: 0, timestampMs: TS, subtype: 'raw', payload: {} },
      },
    ],
  };
}

function ids(messages: AgentUIMessage[]): string[] {
  return messages.map((message) => message.id);
}

// ---------------------------------------------------------------------------
// 可见部件判据：text / reasoning / tool-<name> 三类计入，data-* 与 step-start 不计
// ---------------------------------------------------------------------------

describe('hasVisibleParts：可见部件判据', () => {
  it('正向：text / reasoning / tool-Bash 三类部件各一 → 均为可见部件', () => {
    const cases: [string, Part][] = [
      ['text', textPart('正文')],
      ['reasoning', reasoningPart('先梳理调用链')],
      ['tool-Bash', toolPart('Bash')],
    ];

    for (const [label, part] of cases) {
      expect(hasVisibleParts(assistantWith([part])), label).toBe(true);
    }
  });

  it('正向：text 与 tool 与 reasoning 交错 → 可见；文本为空串的 text 部件仍可见', () => {
    expect(
      hasVisibleParts(assistantWith([textPart('甲'), toolPart('Read'), reasoningPart('乙')])),
    ).toBe(true);
    // 「存在」而非「非空」：空串正文的 text 部件照样产气泡
    expect(hasVisibleParts(assistantWith([textPart('')]))).toBe(true);
  });

  it('异常：只含 data-* 与 step-start 的部件集 → 不可见（实时态空壳消息不产空气泡）', () => {
    const carriers: Part[] = [
      {
        type: 'data-run-started',
        data: { seq: 1, timestampMs: TS, model: null, sessionId: null, tools: [], mcpServers: [] },
      },
      {
        type: 'data-system-notice',
        data: { seq: 2, timestampMs: TS, subtype: 'permission_denial', payload: {} },
      },
      {
        type: 'data-raw',
        data: { seq: 3, timestampMs: TS, eventType: 'mystery', rawJson: '{}' },
      },
      { type: 'step-start' },
    ];

    for (const part of carriers) {
      expect(hasVisibleParts(assistantWith([part])), part.type).toBe(false);
    }
    expect(hasVisibleParts(assistantWith(carriers))).toBe(false);
    expect(hasVisibleParts(assistantWith([]))).toBe(false);
  });

  it('异常：前缀伪装三形态（dynamic-tool / tool / data-tool-x）→ 均不可见', () => {
    const disguises = ['dynamic-tool', 'tool', 'data-tool-x'];

    for (const type of disguises) {
      // 判据是 `tool-` 前缀且排除 `dynamic-tool`，不误吞相邻命名
      expect(hasVisibleParts(assistantWith([dirtyPart(type)])), type).toBe(false);
    }
    // 对照组：真正落在信封内的命名仍可见（排除的是保留字，不是整类）
    expect(hasVisibleParts(assistantWith([toolPart('Read')]))).toBe(true);
    // 空工具名（越签名脏值）按前缀判据仍计入可见——与 agent-blocks 的 isToolPart 同口径
    expect(hasVisibleParts(assistantWith([dirtyPart('tool-')]))).toBe(true);
  });

  it('异常：越过联合类型的脏 part.type（mystery-part）→ 不可见且不抛错', () => {
    expect(() => hasVisibleParts(assistantWith([dirtyPart('mystery-part')]))).not.toThrow();
    expect(hasVisibleParts(assistantWith([dirtyPart('mystery-part')]))).toBe(false);
    // dynamic-tool 不在 tool-<name> 信封内（与 agent-blocks 的 isToolPart 同口径）
    expect(hasVisibleParts(assistantWith([dirtyPart('dynamic-tool')]))).toBe(false);
  });

  it('边界：parts 为空数组 → 不可见（空数组不短路抛错）', () => {
    expect(() => hasVisibleParts(assistantWith([]))).not.toThrow();
    expect(hasVisibleParts(assistantWith([]))).toBe(false);
  });

  it('边界：超长文本（>1000 字符）与含换行 / \\0 / emoji 的部件 → 可见（判据不看长度与字符集）', () => {
    const long = 'x'.repeat(1001);
    const weird = '第一行\n第二行\u0000😀';

    for (const text of [long, weird]) {
      expect(hasVisibleParts(assistantWith([textPart(text)]))).toBe(true);
    }
    expect(long).toHaveLength(1001);
  });
});

// ---------------------------------------------------------------------------
// 过滤判据单点：system 剔除 → 可见部件 → 空内容轮按 metadata.record.status 判定
// ---------------------------------------------------------------------------

describe('filterConversation：对话透镜可见性', () => {
  it('正向：常规多轮会话 → 剔除全部 system、顺序保持、输出为入参非同一引用', () => {
    const all = fold([
      runStartedEvent(0, 1),
      userEvent(1, '问一', 1),
      messageEvent(2, [{ kind: 'text', text: '答一' }], 1),
      userEvent(3, '问二', 2),
      messageEvent(4, [{ kind: 'text', text: '答二' }], 2),
    ]);

    const visible = filterConversation(all);

    expect(all.some((message) => message.role === 'system')).toBe(true);
    expect(visible.some((message) => message.role === 'system')).toBe(false);
    expect(ids(visible)).toEqual(
      all.filter((message) => message.role !== 'system').map((message) => message.id),
    );
    expect(ids(visible)).toHaveLength(4);
    expect(visible).not.toBe(all);
  });

  it('正向：空内容 assistant × 轮行 failed / stopped → 两条均保留，状态原样可读', () => {
    const cases: [string, TurnSummary['status']][] = [
      ['failed', 'failed'],
      ['stopped', 'stopped'],
    ];

    for (const [label, status] of cases) {
      const all = fold([messageEvent(1, [])], [recordRow(status, 2)]);
      const visible = filterConversation(all);

      expect(visible, label).toHaveLength(1);
      expect(visible[0]?.metadata?.record?.status, label).toBe(status);
      expect(visible[0]?.parts, label).toHaveLength(0);
    }
  });

  it('正向：有内容 assistant + failed / stopped 轮行 → 保留且不追加任何判定', () => {
    for (const status of ['failed', 'stopped'] as const) {
      const all = fold(
        [userEvent(0, '问', 1), messageEvent(1, [{ kind: 'text', text: '部分回答' }], 1)],
        [recordRow(status, 1)],
      );
      const visible = filterConversation(all);

      expect(visible).toHaveLength(2);
      expect(visible.map((message) => message.role)).toEqual(['user', 'assistant']);
      expect(visible[1]?.metadata?.record?.status).toBe(status);
    }
  });

  it('异常：空内容 assistant + 轮行 completed → 剔除（无空气泡、无状态行）', () => {
    const all = fold([messageEvent(1, [])], [recordRow('completed', 2)]);

    expect(all).toHaveLength(1);
    expect(filterConversation(all)).toEqual([]);
  });

  it('异常：空内容 assistant + 轮行缺席（record 为 null）→ 剔除（不因状态缺失而放行）', () => {
    const all = fold([messageEvent(1, [])], []);

    expect(all[0]?.metadata?.record).toBeNull();
    expect(filterConversation(all)).toEqual([]);
  });

  it('异常：空内容 assistant + 轮行 running → 剔除（只放行 failed / stopped）', () => {
    const all = fold([messageEvent(1, [])], [recordRow('running', 2)]);

    expect(filterConversation(all)).toEqual([]);
  });

  it('异常：越过类型的非法 status（paused）→ 同 completed 处理，不抛错', () => {
    const all = fold(
      [messageEvent(1, [])],
      [{ ...recordRow('completed', 2), status: 'paused' as TurnSummary['status'] }],
    );
    expect(all[0]?.metadata?.record?.status).toBe('paused');

    expect(() => filterConversation(all)).not.toThrow();
    expect(filterConversation(all)).toEqual([]);
  });

  it('异常：全部为 system 载体消息（runStarted / systemNotice / raw 折叠产物）→ 空数组', () => {
    const all = fold([runStartedEvent(0, 1), systemNoticeEvent(1, 2), rawEvent(2, 3)]);

    expect(all.some((message) => message.role === 'system')).toBe(true);
    // 载体消息的轮内 assistant 亦为 0 部件 + 轮行缺席 ⇒ 全族出局
    expect(filterConversation(all)).toEqual([]);
  });

  it('边界：入参为空数组 → 空数组，不抛错', () => {
    expect(() => filterConversation([])).not.toThrow();
    expect(filterConversation([])).toEqual([]);
  });

  it('边界：单元素三形态 —— 单 system 剔除 / 单有内容 assistant 原样 / 单 user 原样', () => {
    const system = fold([runStartedEvent(0, 1)]);
    expect(system.some((message) => message.role === 'system')).toBe(true);
    expect(filterConversation(system)).toEqual([]);

    const assistant = [assistantWith([textPart('独答')], 'only-assistant')];
    expect(ids(filterConversation(assistant))).toEqual(['only-assistant']);

    const user = [userWith('独问', 'only-user')];
    expect(ids(filterConversation(user))).toEqual(['only-user']);
  });

  it('边界：1000 条混合列表（system / 有内容 / 空内容 completed / 空内容 failed）→ 条数与顺序按判据', () => {
    const all: AgentUIMessage[] = [];
    const expected: string[] = [];
    for (let index = 0; index < 250; index += 1) {
      all.push(systemWith(`sys-${index}`));
      all.push(assistantWith([textPart(`答-${index}`)], `content-${index}`));
      expected.push(`content-${index}`);
      all.push(assistantWith([], `empty-completed-${index}`, 'completed'));
      all.push(assistantWith([], `empty-failed-${index}`, 'failed'));
      expected.push(`empty-failed-${index}`);
    }
    expect(all).toHaveLength(1000);

    const visible = filterConversation(all);

    expect(() => filterConversation(all)).not.toThrow();
    expect(visible).toHaveLength(500);
    expect(ids(visible)).toEqual(expected);
  });

  it('边界：只含被消费孤儿搜索 result 的轮（0 部件 + record 为 null）→ 剔除', () => {
    const orphan = assistantWith([], 'orphan-search');
    expect(orphan.parts).toHaveLength(0);
    expect(orphan.metadata?.record).toBeNull();

    expect(filterConversation([orphan])).toEqual([]);
  });

  it('边界：入参为 undefined（越签名脏值）→ 抛 TypeError，不虚构消息', () => {
    const dirty = undefined as unknown as AgentUIMessage[];

    expect(() => filterConversation(dirty)).toThrow(TypeError);
  });

  it('异常：user 角色的 parts 为空（越签名构造）→ 判据不特判角色，照常剔除', () => {
    // 唯一的角色条件是 system 剔除；user 无可见部件且无 failed / stopped 轮行 ⇒ 出局
    const emptyUser: AgentUIMessage = {
      id: 'user-empty',
      role: 'user',
      metadata: { seq: null, parentToolUseId: null, record: null },
      parts: [],
    };

    expect(filterConversation([emptyUser])).toEqual([]);
    // 对照：同样 0 部件但带 failed 轮行的 user 消息仍保留（判定只读状态，不读角色）
    const failedUser: AgentUIMessage = {
      ...emptyUser,
      id: 'user-failed',
      metadata: { seq: null, parentToolUseId: null, record: recordRow('failed', 1) },
    };
    expect(ids(filterConversation([failedUser]))).toEqual(['user-failed']);
  });
});

// ---------------------------------------------------------------------------
// 吸附层几何：仅 user 索引产条、span 差分、末轮闭合到 totalSize、负值钳制
// ---------------------------------------------------------------------------

describe('userTurnSpans：吸附层占位块几何', () => {
  it('正向：4 条 user 与交替 assistant → index / start / span 逐条等于手工推导值', () => {
    const messages: AgentUIMessage[] = [
      userWith('问一', 'u0'),
      assistantWith([textPart('答一')], 'a1'),
      userWith('问二', 'u2'),
      assistantWith([textPart('答二')], 'a3'),
      userWith('问三', 'u4'),
      assistantWith([textPart('答三')], 'a5'),
      userWith('问四', 'u6'),
    ];
    // 72 / 240 的估值 + 24 行距 + 12 上内边距 ⇒ 每轮跨 360
    const measurements = [12, 96, 360, 444, 720, 804, 1080].map((start) => ({ start }));
    const totalSize = 1080 + 72 + 12;

    const spans = userTurnSpans(messages, measurements, totalSize);

    expect(spans).toEqual([
      { index: 0, start: 12, span: 348 },
      { index: 2, start: 360, span: 360 },
      { index: 4, start: 720, span: 360 },
      { index: 6, start: 1080, span: 84 },
    ]);
    // 末轮闭合：最后一条 span = totalSize − start
    expect(spans.at(-1)?.span).toBe(totalSize - 1080);
  });

  it('正向：首条 user 在索引 0、measurements[0].start 为 paddingStart → 首条 start 取该值', () => {
    const messages: AgentUIMessage[] = [
      userWith('问', 'u0'),
      assistantWith([textPart('答')], 'a1'),
    ];
    const measurements = [{ start: 12 }, { start: 96 }];

    const spans = userTurnSpans(messages, measurements, 96 + 240 + 12);

    expect(spans).toHaveLength(1);
    expect(spans[0]?.start).toBe(12);
    expect(spans[0]?.span).toBe(96 + 240 + 12 - 12);
  });

  it('正向：全部消息皆为 user → 每条 span 取下一条 start 差，末条闭合到 totalSize', () => {
    const messages = Array.from({ length: 4 }, (_, index) => userWith(`问-${index}`, `u${index}`));
    const measurements = [0, 72, 144, 216].map((start) => ({ start }));
    const totalSize = 288;

    const spans = userTurnSpans(messages, measurements, totalSize);

    expect(spans.map((span) => span.index)).toEqual([0, 1, 2, 3]);
    expect(spans.map((span) => span.span)).toEqual([72, 72, 72, 72]);
    const last = spans.at(-1);
    expect((last?.start ?? 0) + (last?.span ?? 0)).toBe(totalSize);
  });

  it('异常：无 user 消息（全 assistant / 全 system）→ 空数组', () => {
    const assistantOnly = [assistantWith([textPart('答')], 'a0')];
    const systemOnly = [systemWith('s0')];

    for (const messages of [assistantOnly, systemOnly]) {
      expect(userTurnSpans(messages, [{ start: 0 }], 100)).toEqual([]);
    }
  });

  it('异常：measurements[index] 缺席（长度小于 messages）→ 该条跳过、末轮退化到 totalSize', () => {
    const messages: AgentUIMessage[] = [
      userWith('问一', 'u0'),
      assistantWith([textPart('答')], 'a1'),
      userWith('问二', 'u2'),
    ];
    // 只测到索引 0，索引 2 的 user 无测量 ⇒ 跳过；索引 0 的下一 user 不在列表内 ⇒ 闭合到 totalSize
    const measurements = [{ start: 12 }];

    const spans = userTurnSpans(messages, measurements, 500);

    expect(spans).toEqual([{ index: 0, start: 12, span: 488 }]);
    expect(() => userTurnSpans(messages, measurements, 500)).not.toThrow();
  });

  it('异常：totalSize < start（末轮倒挂）→ span 钳制为 0，不产负高度', () => {
    const messages: AgentUIMessage[] = [userWith('问', 'u0')];

    const spans = userTurnSpans(messages, [{ start: 900 }], 120);

    expect(spans).toEqual([{ index: 0, start: 900, span: 0 }]);
    expect(spans[0]?.span).toBeGreaterThanOrEqual(0);
  });

  it('边界：messages 为空数组 / measurements 为空数组（含 user）→ 空数组', () => {
    expect(userTurnSpans([], [], 0)).toEqual([]);
    // measurements 全缺席 ⇒ 全部跳过（与「measurements[index] 缺席」同一条护栏）
    expect(userTurnSpans([userWith('问', 'u0')], [], 100)).toEqual([]);
  });

  it('边界：totalSize 为 0 且单条 user 的 start 为 0 → span 为 0，index / start 正确', () => {
    const spans = userTurnSpans([userWith('问', 'u0')], [{ start: 0 }], 0);

    expect(spans).toEqual([{ index: 0, start: 0, span: 0 }]);
    expect(spans[0]?.span).toBeGreaterThanOrEqual(0);
  });

  it('边界：start 极值三形态（0 / MAX_SAFE_INTEGER / -1）→ 差值计算不抛错、span 非负', () => {
    const cases: [number, number][] = [
      [0, 100],
      [Number.MAX_SAFE_INTEGER, 0],
      [-1, 100],
    ];

    for (const [start, totalSize] of cases) {
      const spans = userTurnSpans([userWith('问', 'u0')], [{ start }], totalSize);
      expect(spans[0]?.start, String(start)).toBe(start);
      expect(spans[0]?.span, String(start)).toBeGreaterThanOrEqual(0);
    }
    // 负 start 不参与夹取钳制，只作用于 span
    expect(userTurnSpans([userWith('问', 'u0')], [{ start: -1 }], 100)).toEqual([
      { index: 0, start: -1, span: 101 },
    ]);
  });

  it('边界：totalSize 极值三形态（0 / -1 / MAX_SAFE_INTEGER）→ 差值计算不抛错、span 非负', () => {
    const cases: number[] = [0, -1, Number.MAX_SAFE_INTEGER];

    for (const totalSize of cases) {
      const spans = userTurnSpans([userWith('问', 'u0')], [{ start: 5 }], totalSize);
      expect(spans, String(totalSize)).toHaveLength(1);
      expect(spans[0]?.span, String(totalSize)).toBeGreaterThanOrEqual(0);
    }
    // 倒挂与极小值一律钳到 0（MUST NOT 产出负高度），极大值照常外推
    expect(userTurnSpans([userWith('问', 'u0')], [{ start: 5 }], -1)).toEqual([
      { index: 0, start: 5, span: 0 },
    ]);
    expect(userTurnSpans([userWith('问', 'u0')], [{ start: 5 }], Number.MAX_SAFE_INTEGER)).toEqual([
      { index: 0, start: 5, span: Number.MAX_SAFE_INTEGER - 5 },
    ]);
  });

  it('边界：messages / measurements 为 undefined（越签名脏值）→ 抛 TypeError，不虚构占位块', () => {
    const dirtyMessages = undefined as unknown as AgentUIMessage[];
    const dirtyMeasurements = undefined as unknown as { start: number }[];

    expect(() => userTurnSpans(dirtyMessages, [{ start: 5 }], 100)).toThrow(TypeError);
    expect(() => userTurnSpans([userWith('问', 'u0')], dirtyMeasurements, 100)).toThrow(TypeError);
  });

  it('边界：index 严格递增、start 单调不减（吸附层与虚拟项同基准的机械判据）', () => {
    const messages: AgentUIMessage[] = [
      assistantWith([textPart('答')], 'a0'),
      userWith('问一', 'u1'),
      userWith('问二', 'u2'),
      assistantWith([textPart('答')], 'a3'),
      userWith('问三', 'u4'),
    ];
    const measurements = [0, 24, 120, 216, 264].map((start) => ({ start }));

    const spans = userTurnSpans(messages, measurements, 600);

    expect(spans.map((span) => span.index)).toEqual([1, 2, 4]);
    for (let index = 1; index < spans.length; index += 1) {
      const prev = spans[index - 1];
      const curr = spans[index];
      expect(curr?.index ?? 0).toBeGreaterThan(prev?.index ?? 0);
      expect(curr?.start ?? 0).toBeGreaterThanOrEqual(prev?.start ?? 0);
    }
  });

  it('边界：1000 条中 100 条 user → 输出 100 条、末条闭合到 totalSize', () => {
    const messages: AgentUIMessage[] = [];
    const measurements: { start: number }[] = [];
    let cursor = 12;
    for (let index = 0; index < 1000; index += 1) {
      const isUser = index % 10 === 0;
      messages.push(
        isUser
          ? userWith(`问-${index}`, `u-${index}`)
          : assistantWith([textPart('答')], `a-${index}`),
      );
      measurements.push({ start: cursor });
      cursor += (isUser ? 72 : 240) + 24;
    }
    const totalSize = cursor;

    const spans = userTurnSpans(messages, measurements, totalSize);

    expect(spans).toHaveLength(100);
    expect(spans.at(-1)?.index).toBe(990);
    expect((spans.at(-1)?.start ?? 0) + (spans.at(-1)?.span ?? 0)).toBe(totalSize);
  });
});

// ---------------------------------------------------------------------------
// 会话身份（贴底复位键）：取首条携 record 的消息的 sessionId，全体缺席为 null
// ---------------------------------------------------------------------------

/** 指定会话 id 的轮行（`sessionId` 是被测身份的读取面） */
function sessionRow(sessionId: string, status: TurnSummary['status'] = 'completed'): TurnSummary {
  return { ...recordRow(status, 1), sessionId };
}

/** 携指定会话 id 的 assistant 消息（有可见部件，保证过得了过滤判据） */
function assistantOfSession(sessionId: string, id: string): AgentUIMessage {
  const base = assistantWith([textPart(`答-${id}`)], id);
  return {
    ...base,
    metadata: { seq: null, parentToolUseId: null, record: sessionRow(sessionId) },
  };
}

describe('conversationIdentity：贴底复位键', () => {
  it('正向：多轮列表各条 sessionId 同值 → 返回该值（身份不随条数变化）', () => {
    const all = fold(
      [
        userEvent(0, '问一', 1),
        messageEvent(1, [{ kind: 'text', text: '答一' }], 1),
        userEvent(2, '问二', 2),
        messageEvent(3, [{ kind: 'text', text: '答二' }], 2),
      ],
      [sessionRow('S'), { ...sessionRow('S'), turnId: 2 }],
    );

    expect(conversationIdentity(all)).toBe('S');
  });

  it('正向：首条消息无 record（未收口）、第 2 条起有 → 返回首个携 record 那条的会话 id', () => {
    const streamed = assistantWith([textPart('流式首轮')], 'a0');
    expect(streamed.metadata?.record ?? null).toBeNull();

    const all: AgentUIMessage[] = [streamed, userWith('问', 'u1'), assistantOfSession('S9', 'a2')];

    expect(conversationIdentity(all)).toBe('S9');
  });

  it('正向：同会话尾部追加一条 → 返回值不变（AC-13「同会话追加不复位」的纯函数半边）', () => {
    const base: AgentUIMessage[] = [assistantOfSession('S', 'a0'), userWith('问', 'u1')];
    const appended = [...base, assistantOfSession('S', 'a2')];

    expect(conversationIdentity(base)).toBe('S');
    expect(conversationIdentity(appended)).toBe(conversationIdentity(base));
  });

  it('正向：同一消息流式增长（parts 由 1 变 3、record 不变）→ 返回值不变', () => {
    const grown: AgentUIMessage = {
      ...assistantOfSession('S', 'a0'),
      parts: [textPart('甲'), reasoningPart('乙'), toolPart('Bash')],
    };

    expect(conversationIdentity([grown])).toBe('S');
    expect(conversationIdentity([{ ...grown, parts: [textPart('甲')] }])).toBe('S');
  });

  it('异常：换会话（sessionId 由 A 变 B、条数相同）→ 返回值变化且等于 B', () => {
    const sessionA: AgentUIMessage[] = [
      assistantOfSession('A', 'a0'),
      assistantOfSession('A', 'a1'),
    ];
    const sessionB: AgentUIMessage[] = [
      assistantOfSession('B', 'a0'),
      assistantOfSession('B', 'a1'),
    ];

    expect(conversationIdentity(sessionA)).toBe('A');
    expect(conversationIdentity(sessionB)).toBe('B');
    expect(conversationIdentity(sessionB)).not.toBe(conversationIdentity(sessionA));
  });

  it('异常：record 为 null / metadata 缺席 / record 为 undefined 三形态 → 均返回 null', () => {
    const variants: (AgentUIMessage['metadata'] | undefined)[] = [
      { seq: null, parentToolUseId: null, record: null },
      undefined,
      { seq: null, parentToolUseId: null },
    ];

    for (const metadata of variants) {
      const all: AgentUIMessage[] = [
        { id: 'a0', role: 'assistant', metadata, parts: [textPart('答')] },
      ];
      expect(conversationIdentity(all), JSON.stringify(metadata)).toBeNull();
    }
  });

  it('边界：入参为空数组 → null（首帧前的空集合不产生身份）', () => {
    expect(() => conversationIdentity([])).not.toThrow();
    expect(conversationIdentity([])).toBeNull();
  });

  it('边界：sessionId 为空串（越签名脏值）→ 返回空串而非 null，不抛错', () => {
    const all = [assistantOfSession('', 'a0')];

    expect(() => conversationIdentity(all)).not.toThrow();
    // 实现按 record 存在性判「已知」：空串是已知的键（不等于「未知」的 null）
    expect(conversationIdentity(all)).toBe('');
  });

  it('边界：sessionId 超长 / 含换行与 emoji → 原样返回，不截断不改写', () => {
    const long = 'x'.repeat(1001);
    const weird = '会话\n一\u0000😀';

    for (const sessionId of [long, weird]) {
      expect(conversationIdentity([assistantOfSession(sessionId, 'a0')])).toBe(sessionId);
    }
  });

  it('边界：1000 条列表 → 取首条命中即返回（首条已带 record），不抛错', () => {
    const all: AgentUIMessage[] = Array.from({ length: 1000 }, (_, index) =>
      assistantOfSession('S-first', `a-${index}`),
    );

    expect(() => conversationIdentity(all)).not.toThrow();
    expect(conversationIdentity(all)).toBe('S-first');
  });

  it('边界：入参为 undefined（越签名脏值）→ 抛 TypeError，不虚构会话身份', () => {
    const dirty = undefined as unknown as AgentUIMessage[];

    expect(() => conversationIdentity(dirty)).toThrow(TypeError);
  });
});
