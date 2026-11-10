import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { foldSessionMessages, type AgentUIMessage } from '../../lib/agent-adapter';
import type { AgentBlock, AgentEvent, TurnSummary } from '../../types/dto';
import { AgentMessages } from './agent-messages';

// ---------------------------------------------------------------------------
// 纯渲染组件，消费方注入 AgentUIMessage[]（由适配层真实折叠输出构造 fixture），
// 无进程边界，不需要 Mock。
// ---------------------------------------------------------------------------

const TS = 1727000000000;
/** 单轮 fixture 的分组键（消息 id 锚点派生自它） */
const TURN = 1;

function message(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
  turnId: number = TURN,
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

function userMessage(seq: number, text: string, turnId: number = TURN): AgentEvent {
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

function runStarted(seq: number, turnId: number = TURN): AgentEvent {
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

function systemNotice(seq: number, turnId: number = TURN): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'systemNotice',
    subtype: 'permission_denial',
    payload: {},
  };
}

function raw(seq: number, turnId: number = TURN): AgentEvent {
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
  turnId: number = TURN,
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

/** 折叠唯一入口的薄包装（本套件只关心呈现层；轮行按需给） */
function fold(events: AgentEvent[], turns: TurnSummary[] = []): AgentUIMessage[] {
  return foldSessionMessages(events, turns);
}

/** 对话气泡基线：单轮（提问 + 思考/正文 + 工具对卡）→ 两条气泡 */
function bubbleEvents(): AgentEvent[] {
  return [
    userMessage(0, '帮我看看重试逻辑'),
    message(1, [
      { kind: 'thinking', thinking: '先梳理调用链' },
      { kind: 'text', text: '**重试**采用指数退避' },
      { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
      { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
    ]),
  ];
}

// ---------------------------------------------------------------------------
// 四变体气泡与注册表特化卡
// ---------------------------------------------------------------------------

describe('AgentMessages：四变体气泡', () => {
  it('user 文本、assistant 文本、reasoning（思考折叠）、tool 对卡四类部件分别呈现', async () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    const bubbles = screen.getAllByTestId('chat-bubble');
    expect(bubbles).toHaveLength(2);
    expect(bubbles.map((bubble) => bubble.getAttribute('data-role'))).toEqual([
      'user',
      'assistant',
    ]);

    const texts = screen.getAllByTestId('block-text');
    expect(texts).toHaveLength(2);
    expect(texts[0]?.textContent).toContain('帮我看看重试逻辑');
    expect(texts[1]?.textContent).toContain('重试');

    // 思考块默认折叠：触发按钮文案在场，正文点开后可见
    const thinking = screen.getByTestId('block-thinking');
    expect(thinking.textContent).toContain('思考');
    const thinkingTrigger = within(thinking).getByRole('button');
    await (async () => {
      act(() => {
        fireEvent.click(thinkingTrigger);
      });
    })();
    expect(thinking.textContent).toContain('先梳理调用链');

    const toolCard = screen.getByTestId('block-tool-use');
    expect(toolCard.getAttribute('data-tool-id')).toBe('tu_1');
    // 结果正文在卡面展开后挂载（TraceCollapsible 默认折叠）
    fireEvent.click(within(toolCard).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toContain('目录内容');
  });

  it('user 气泡为整宽 tinted 泡、assistant 无泡底（对齐属性迁移基线）', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    const bubbles = screen.getAllByTestId('chat-bubble');
    // user 泡改整宽表达，不再靠 align 属性撑右对齐
    expect(bubbles[0]?.getAttribute('data-align')).toBe('start');
    expect(bubbles[1]?.getAttribute('data-align')).toBe('start');
    // 内容块底色：user 泡走 Bubble tinted，assistant 泡无泡底
    const userBubble = bubbles[0]?.querySelector('[data-slot="bubble"]');
    expect(userBubble?.getAttribute('data-variant')).toBe('tinted');
    expect(userBubble?.querySelector('[data-slot="bubble-content"]')).toBeTruthy();
    expect(bubbles[1]?.querySelector('[data-slot="bubble"]')).toBeNull();
  });

  it('一轮一条气泡：同轮两次响应（多段密封块）合成单个 assistant 气泡', () => {
    const events: AgentEvent[] = [
      userMessage(0, '问'),
      message(1, [{ kind: 'text', text: '第一段' }]),
      message(2, [{ kind: 'text', text: '第二段' }]),
    ];

    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    const bubbles = screen.getAllByTestId('chat-bubble');
    expect(bubbles).toHaveLength(2);
    const assistantTexts = within(bubbles[1] as HTMLElement)
      .getAllByTestId('block-text')
      .map((node) => node.textContent);
    expect(assistantTexts).toEqual(['第一段', '第二段']);
  });

  it('注册表特化卡（AskUserQuestion 静态卡）：命中注册表即渲染专属卡而非默认对卡', () => {
    const events: AgentEvent[] = [
      message(1, [
        {
          kind: 'toolUse',
          id: 'tu_ask',
          name: 'AskUserQuestion',
          input: {
            questions: [
              {
                question: '选择重试策略？',
                options: [
                  { label: '指数退避', description: '推荐' },
                  { label: '固定间隔', description: null },
                ],
              },
            ],
          },
        },
      ]),
    ];
    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    expect(screen.getByTestId('ask-question-card').textContent).toContain('AskUserQuestion');
    expect(screen.getByTestId('ask-question').textContent).toBe('选择重试策略？');
    const options = screen.getAllByTestId('ask-option');
    expect(options).toHaveLength(2);
    expect(options[0]?.textContent).toContain('指数退避');
    expect(options[0]?.textContent).toContain('推荐');
    expect(options[1]?.textContent).toContain('固定间隔');
  });
});

// ---------------------------------------------------------------------------
// system 消息过滤（辅助行 / raw / record 不在对话透镜呈现）
// ---------------------------------------------------------------------------

describe('AgentMessages：system 消息过滤', () => {
  it('入参含 system 载体消息（run-started / system-notice / raw / run-result）→ 整条不进渲染列表', () => {
    const carrierMessages = fold([
      runStarted(10, 10),
      systemNotice(11, 11),
      raw(12, 12),
      {
        seq: 13,
        turnId: 13,
        timestampMs: TS,
        kind: 'turnDone',
        subtype: 'success',
        isError: false,
        numTurns: 1,
        durationMs: 1,
        costUsd: null,
        usage: null,
        sessionId: 's-1',
      },
    ]);
    const bubbles = fold(bubbleEvents());
    const messages = [...bubbles, ...carrierMessages];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    // 载体消息的 data 部件不出现在对话透镜
    expect(screen.queryByTestId('event-run-started')).toBeNull();
    expect(screen.queryByTestId('event-system')).toBeNull();
    expect(screen.queryByTestId('event-raw')).toBeNull();
    expect(screen.queryByTestId('event-run-result')).toBeNull();
    // user / assistant 气泡不受影响
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(2);
    // 过滤不回写入参组
    expect(messages.length).toBeGreaterThan(bubbles.length);
  });
});

// ---------------------------------------------------------------------------
// 容器组合与状态呈现
// ---------------------------------------------------------------------------

describe('AgentMessages：容器组合', () => {
  it('message-scroller 组合与滚动容器结构与迁移前一致（Provider + 视口 + 到底按钮）', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    const section = screen.getByTestId('agent-messages');
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(section.className).toContain(className);
    }
    expect(document.querySelector('[data-slot="message-scroller"]') !== null).toBe(true);
    const viewport = document.querySelector('[data-slot="message-scroller-viewport"]');
    expect(viewport?.getAttribute('aria-label')).toBe('对话消息');
    expect(document.querySelector('[data-slot="message-scroller-button"]') !== null).toBe(true);
    // 每条气泡一个 Item（消息状态留应用层的组合基线）
    expect(document.querySelectorAll('[data-slot="message-scroller-item"]')).toHaveLength(2);
  });
});

describe('AgentMessages：空态与运行中态', () => {
  it('messages 为空 → empty 态呈现，不抛错', () => {
    render(<AgentMessages messages={[]} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryByTestId('chat-bubble')).toBeNull();
  });

  it('全为 system 载体消息（过滤后为空）→ empty 态呈现（判定基于过滤后列表）', () => {
    const messages = fold([runStarted(0, 1), systemNotice(1, 2)], []);
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryByTestId('chat-bubble')).toBeNull();
  });

  it('loading 且可见列表为空 → 会话还原中态', () => {
    render(<AgentMessages messages={[]} loading={true} running={false} />);

    expect(screen.getByTestId('agent-messages-loading') !== null).toBe(true);
  });

  it('loading 但已有可见气泡 → 不呈现还原中态，气泡照常呈现', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={true} running={false} />);

    expect(screen.queryByTestId('agent-messages-loading')).toBeNull();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(2);
  });

  it('running 且无终态 record → 头部运行中标记呈现', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={true} />);

    expect(screen.getByTestId('agent-messages-running') !== null).toBe(true);
    expect(within(screen.getByTestId('agent-messages')).getByText('运行中…') !== null).toBe(true);
  });

  it('非运行态无运行中标记', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    expect(screen.queryByTestId('agent-messages-running')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 历史分页：默认最新一页，触顶装载更早（prepend 后的滚动锚定由
// message-scroller 原语的 preserveScrollOnPrepend 承担，此处只测自研分页层）
// ---------------------------------------------------------------------------

/** 交替 user/assistant 的 N 条对话（每对消息一轮，轮行按 completed 给） */
function pagedConversation(count: number, seqOffset = 0): AgentUIMessage[] {
  const events: AgentEvent[] = [];
  const turns: TurnSummary[] = [];
  for (let i = 0; i < count; i += 2) {
    const turnId = seqOffset + i + 1;
    turns.push(recordRow('completed', turnId));
    events.push(userMessage(seqOffset + i, `user-msg-${i}`, turnId));
    if (i + 1 < count) {
      events.push(
        message(seqOffset + i + 1, [{ kind: 'text', text: `assistant-msg-${i}` }], null, turnId),
      );
    }
  }
  return fold(events, turns);
}

/** jsdom 无 IntersectionObserver：仅记录装配参数，触顶由 trigger 显式驱动 */
class FakeIntersectionObserver {
  public static instances: FakeIntersectionObserver[] = [];

  constructor(
    private readonly callback: (entries: { isIntersecting: boolean }[]) => void,
    public readonly init?: { root?: Element | null; rootMargin?: string },
  ) {
    FakeIntersectionObserver.instances.push(this);
  }

  public observe(): void {}

  public unobserve(): void {}

  public takeRecords(): never[] {
    return [];
  }

  public disconnect(): void {
    FakeIntersectionObserver.instances = FakeIntersectionObserver.instances.filter(
      (io) => io !== this,
    );
  }

  public trigger(isIntersecting: boolean): void {
    this.callback([{ isIntersecting }]);
  }
}

function triggerReachTop(): void {
  const active = FakeIntersectionObserver.instances.at(-1);
  if (active === undefined) {
    throw new Error('触顶触发时无活跃观察器');
  }
  act(() => {
    active.trigger(true);
  });
}

describe('AgentMessages：历史分页', () => {
  beforeEach(() => {
    FakeIntersectionObserver.instances = [];
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('默认仅渲染最新一页（10 条），更早消息不进 DOM，顶部提示加载中…', () => {
    render(<AgentMessages messages={pagedConversation(25)} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    const joined = screen
      .getAllByTestId('block-text')
      .map((node) => node.textContent)
      .join('\n');
    expect(joined).toContain('user-msg-24'); // 最新一条在
    expect(joined).toContain('assistant-msg-14'); // 当前页最早一条
    expect(joined).not.toContain('user-msg-14'); // 上一页及更早不渲染
    expect(screen.getByTestId('history-load-hint').textContent).toContain('加载中…');
  });

  it('触顶连续装载：每触发一次增一页，装满后转终态并撤销观察器', () => {
    render(<AgentMessages messages={pagedConversation(25)} loading={false} running={false} />);

    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(20);

    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(25);
    expect(screen.queryByTestId('history-load-hint')).toBeNull();
    // 已到最早 → 不再保持任何触顶观察器
    expect(FakeIntersectionObserver.instances).toHaveLength(0);
  });

  it('观察器 root 绑定消息滚动视口，rootMargin 形成临近顶部预载区', () => {
    render(<AgentMessages messages={pagedConversation(25)} loading={false} running={false} />);

    const io = FakeIntersectionObserver.instances.at(-1);
    expect(io?.init?.root).toBe(document.querySelector('[data-slot="message-scroller-viewport"]'));
    expect(io?.init?.rootMargin).toBe('300px 0px 0px 0px');
  });

  it('system 载体撑大入参不计入分页：可见消息足一页 → 无提示行、无观察器', () => {
    const messages = [
      ...pagedConversation(10),
      ...fold([runStarted(90, 90), systemNotice(91, 91), raw(92, 92)]),
    ];
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    expect(screen.queryByTestId('history-load-hint')).toBeNull();
    expect(screen.queryByTestId('history-load-complete')).toBeNull();
    expect(FakeIntersectionObserver.instances).toHaveLength(0);
  });

  it('换会话（头部消息变化）→ 分页窗口复位回默认最新一页', () => {
    const { rerender } = render(
      <AgentMessages messages={pagedConversation(25)} loading={false} running={false} />,
    );
    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(20);

    rerender(
      <AgentMessages messages={pagedConversation(25, 100)} loading={false} running={false} />,
    );
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    expect(screen.getByTestId('history-load-hint').textContent).toContain('加载中…');
  });
});

// ---------------------------------------------------------------------------
// 空内容轮的可见性判据（AC-9 / D15）：按 metadata.record.status 判定
// ---------------------------------------------------------------------------

/** `parts: []` 的 assistant 消息（零内容轮形态；`record` 决定可见性） */
function emptyTurnMessage(turnId: number, status: TurnSummary['status'] | null): AgentUIMessage {
  return {
    id: `turn-${turnId}`,
    role: 'assistant',
    metadata: {
      seq: null,
      parentToolUseId: null,
      record: status === null ? null : recordRow(status, turnId),
    },
    parts: [],
  };
}

describe('AgentMessages：空内容轮可见性判据（AC-9）', () => {
  beforeEach(() => {
    FakeIntersectionObserver.instances = [];
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('1 条空 parts + completed 轮 + 2 条正常消息 → 恰 2 个 chat-bubble（不渲染空气泡）', () => {
    const messages: AgentUIMessage[] = [
      ...fold(
        [userMessage(1, '第一问'), message(2, [{ kind: 'text', text: '第一答' }])],
        [recordRow('completed')],
      ),
      emptyTurnMessage(42, 'completed'),
    ];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(2);
    expect(document.body.textContent).toContain('第一问');
    expect(document.body.textContent).toContain('第一答');
  });

  it('分页不计不可见空内容轮：11 条正常 + 5 条空 + completed → 一页 10 条且提示行在场', () => {
    const messages = [
      ...pagedConversation(11),
      ...Array.from({ length: 5 }, (_, index) => emptyTurnMessage(500 + index, 'completed')),
    ];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    expect(screen.getByTestId('history-load-hint').textContent).toContain('加载中…');

    // 触顶装载按过滤后列表递增（装载后 11 条，提示行撤销，观察器归零）
    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(11);
    expect(screen.queryByTestId('history-load-hint')).toBeNull();
    // loadMore 幂等：再触发不增条
    expect(FakeIntersectionObserver.instances).toHaveLength(0);
  });

  it('分页计入可见的失败 / 停止空内容轮：10 条正常 + 1 条空 + failed → 一页 10 条 + 提示行在场', () => {
    const messages = [...pagedConversation(10), emptyTurnMessage(600, 'failed')];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    // 状态可读的轮参与计数 ⇒ 过滤后 11 条 > 一页
    expect(screen.getByTestId('history-load-hint').textContent).toContain('加载中…');
    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(11);
    expect(screen.getByTestId('turn-status-line')).toBeTruthy();
  });

  it('只含被消费孤儿搜索 result 的消息（折叠后 0 部件、record 为 null）不进对话透镜', () => {
    const events: AgentEvent[] = [
      userMessage(1, '帮我搜'),
      message(2, [
        {
          kind: 'webSearch',
          id: 'srvtoolu_ghost',
          query: '查询',
          results: [],
          error: null,
          rawUse: { type: 'server_tool_use', id: 'srvtoolu_ghost' },
          rawResult: null,
        },
      ]),
      // 第二段：仅被 use 消费的 result 侧块 → 折叠后 parts.length === 0
      message(3, [
        {
          kind: 'webSearch',
          id: 'srvtoolu_ghost',
          query: '',
          results: [{ url: 'https://ghost/1', title: '幽灵来源', pageAge: null }],
          error: null,
          rawUse: null,
          rawResult: { type: 'web_search_tool_result', tool_use_id: 'srvtoolu_ghost' },
        },
      ]),
    ];
    // 无轮行 ⇒ record 为 null；该轮 assistant 有搜索卡一个部件，故仍在透镜内
    const messages = fold(events);
    expect(messages.find((m) => m.role === 'assistant')?.parts).toHaveLength(1);

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// 轮内状态行（AC-9 / D16）：failed / stopped 空内容轮
// ---------------------------------------------------------------------------

describe('AgentMessages：轮内状态行', () => {
  it('空 parts + failed（error 携原文）→ 状态行 + 通用标题 + 可展开原文；无空白气泡叠加', () => {
    const messages: AgentUIMessage[] = [emptyTurnMessage(7, 'failed')];
    messages[0] = {
      ...(messages[0] as AgentUIMessage),
      metadata: {
        seq: null,
        parentToolUseId: null,
        record: recordRow('failed', 7, '进程结束但未产出收敛事件'),
      },
    };

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    const line = screen.getByTestId('turn-status-line');
    expect(line.getAttribute('data-status')).toBe('failed');
    expect(screen.getByTestId('turn-status-line-failed').textContent).toBe('Agent 运行失败');
    expect(screen.getByTestId('turn-status-line-raw').textContent).toContain(
      '进程结束但未产出收敛事件',
    );
    // 状态行取代部件列表：该气泡内无文本块
    const bubble = line.closest('[data-testid="chat-bubble"]');
    expect(bubble).not.toBeNull();
    expect(within(bubble as HTMLElement).queryAllByTestId('block-text')).toHaveLength(0);
  });

  it('空 parts + stopped（error 为 null）→ 「已停止」状态行，无原文 details', () => {
    const messages: AgentUIMessage[] = [emptyTurnMessage(8, 'stopped')];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    const line = screen.getByTestId('turn-status-line');
    expect(line.getAttribute('data-status')).toBe('stopped');
    expect(screen.getByTestId('turn-status-line-stopped').textContent).toBe('已停止');
    expect(screen.queryByTestId('turn-status-line-raw')).toBeNull();
  });

  it('parts 非空 + stopped → 只呈正常气泡，状态行缺席（不叠加）', () => {
    const messages = fold(
      [userMessage(0, '问'), message(1, [{ kind: 'text', text: '部分回答' }])],
      [recordRow('stopped')],
    );

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.queryByTestId('turn-status-line')).toBeNull();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(2);
  });

  it('parts 非空 + failed → 只呈正常气泡，状态行缺席（轮失败事实仍可从 metadata 读出）', () => {
    const messages = fold(
      [message(1, [{ kind: 'text', text: '部分回答' }])],
      [recordRow('failed', TURN, 'provider 认证失败')],
    );

    expect(messages[0]?.metadata?.record?.status).toBe('failed');
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.queryByTestId('turn-status-line')).toBeNull();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(1);
  });

  it('原文来源四形态：仅 record.error / 仅 api_error 通知 / 两者去重 / 两者皆空', () => {
    const apiError = (payload: unknown): AgentUIMessage['parts'][number] => ({
      type: 'data-system-notice',
      data: { seq: 9, timestampMs: TS, subtype: 'api_error', payload },
    });

    const cases: [string, TurnSummary | null, AgentUIMessage['parts'], string | null][] = [
      ['仅 record.error', recordRow('failed', 7, '记录侧原文'), [], '记录侧原文'],
      [
        '仅 api_error 通知',
        { ...recordRow('failed', 7, null), error: null },
        [apiError({ error: '通知侧原文' })],
        '通知侧原文',
      ],
      [
        '两者去重',
        recordRow('failed', 7, '同一句话'),
        [apiError({ error: '同一句话' })],
        '同一句话',
      ],
      ['两者皆空', { ...recordRow('failed', 7, null), error: null }, [], null],
    ];

    for (const [label, record, parts, expected] of cases) {
      const messages: AgentUIMessage[] = [
        {
          id: 'turn-7',
          role: 'assistant',
          metadata: { seq: null, parentToolUseId: null, record },
          parts,
        },
      ];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(screen.getByTestId('turn-status-line'), label).toBeTruthy();
      if (expected === null) {
        expect(screen.queryByTestId('turn-status-line-raw'), label).toBeNull();
      } else {
        expect(screen.getByTestId('turn-status-line-raw').textContent, label).toBe(expected);
      }
      unmount();
    }
  });

  it('api_error 通知的 payload.error 非字符串 / 空白 / payload 非对象 → 跳过该来源，不挂空 details', () => {
    const variants: unknown[] = [
      { error: { nested: 1 } },
      { error: 42 },
      { error: '   ' },
      '非对象',
    ];
    for (const payload of variants) {
      const messages: AgentUIMessage[] = [
        {
          id: 'turn-7',
          role: 'assistant',
          metadata: {
            seq: null,
            parentToolUseId: null,
            record: { ...recordRow('failed', 7, null), error: null },
          },
          parts: [
            {
              type: 'data-system-notice',
              data: { seq: 9, timestampMs: TS, subtype: 'api_error', payload },
            },
          ],
        },
      ];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(screen.getByTestId('turn-status-line')).toBeTruthy();
      expect(screen.queryByTestId('turn-status-line-raw')).toBeNull();
      unmount();
    }
  });

  it('可见性判据单一来源：空 parts + failed 既计入分页又只渲染一次状态行', () => {
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
    FakeIntersectionObserver.instances = [];
    try {
      const messages = [...pagedConversation(10), emptyTurnMessage(700, 'failed')];
      render(<AgentMessages messages={messages} loading={false} running={false} />);

      // 过滤保留（计入分页 ⇒ 有提示行）
      expect(screen.getByTestId('history-load-hint')).toBeTruthy();
      triggerReachTop();
      // 渲染恰一次（不出现两处各渲染一次）
      expect(screen.getAllByTestId('turn-status-line')).toHaveLength(1);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('边界：只有 data-* / step-start 部件的 assistant 视为无可见部件；status 决定是否呈状态行', () => {
    const partVariants: AgentUIMessage['parts'][] = [
      [{ type: 'data-raw', data: { seq: 1, timestampMs: TS, eventType: 'x', rawJson: '{}' } }],
      [{ type: 'step-start' }],
    ];

    for (const parts of partVariants) {
      for (const status of ['completed', 'running', 'failed', 'stopped'] as const) {
        const messages: AgentUIMessage[] = [
          {
            id: 'turn-9',
            role: 'assistant',
            metadata: { seq: null, parentToolUseId: null, record: recordRow(status, 9) },
            parts,
          },
        ];
        const { unmount } = render(
          <AgentMessages messages={messages} loading={false} running={false} />,
        );
        const shown = status === 'failed' || status === 'stopped';
        expect(screen.queryAllByTestId('turn-status-line')).toHaveLength(shown ? 1 : 0);
        expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(shown ? 1 : 0);
        unmount();
      }
    }
  });

  it('异常：metadata 缺席 / record 为 undefined / 为 null → 不抛错，空 parts 不渲染', () => {
    const variants: (AgentUIMessage['metadata'] | undefined)[] = [
      undefined,
      { seq: 0, parentToolUseId: null },
      { seq: 0, parentToolUseId: null, record: null },
    ];

    for (const metadata of variants) {
      const messages: AgentUIMessage[] = [{ id: 'turn-1', role: 'assistant', metadata, parts: [] }];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(screen.queryByTestId('turn-status-line')).toBeNull();
      expect(screen.queryByTestId('chat-bubble')).toBeNull();
      unmount();
    }
  });

  it('异常：record.status 越过类型的未来枚举 / 脏串 → 按非 failed 非 stopped 处理（不渲染、不抛错）', () => {
    for (const status of ['futureStatus', '', 'FAILED']) {
      const messages: AgentUIMessage[] = [
        {
          id: 'turn-1',
          role: 'assistant',
          metadata: {
            seq: null,
            parentToolUseId: null,
            record: { ...recordRow('completed'), status: status as TurnSummary['status'] },
          },
          parts: [],
        },
      ];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(screen.queryByTestId('turn-status-line')).toBeNull();
      expect(screen.queryByTestId('chat-bubble')).toBeNull();
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// 一轮一条气泡（呈现侧）与 pause_turn 搜索两段合轮（AC-1 / AC-10）
// ---------------------------------------------------------------------------

describe('AgentMessages：一轮一条气泡与搜索两段合轮（AC-1 / AC-10）', () => {
  it('同一轮 text + reasoning + tool 三部件 → 恰 1 个 assistant 气泡，部件按序呈现', () => {
    const events: AgentEvent[] = [
      userMessage(0, '一轮多部件'),
      message(1, [
        { kind: 'thinking', thinking: '先想' },
        { kind: 'text', text: '再答' },
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
      ]),
    ];

    render(
      <AgentMessages
        messages={fold(events, [recordRow('completed')])}
        loading={false}
        running={false}
      />,
    );

    const assistants = screen
      .getAllByTestId('chat-bubble')
      .filter((bubble) => bubble.getAttribute('data-role') === 'assistant');
    expect(assistants).toHaveLength(1);
    const bubble = assistants[0] as HTMLElement;
    expect(within(bubble).getByTestId('block-thinking')).toBeTruthy();
    expect(within(bubble).getByTestId('block-text').textContent).toBe('再答');
    expect(within(bubble).getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe('tu_1');
  });

  it('pause_turn 两段同轮同消息：同一气泡内搜索卡呈查询词与来源列表，无第二条气泡', () => {
    const events: AgentEvent[] = [
      userMessage(0, '帮我查今日汇率'),
      // use 侧段
      message(1, [
        {
          kind: 'webSearch',
          id: 'srvtoolu_1',
          query: '今日汇率',
          results: [],
          error: null,
          rawUse: { type: 'server_tool_use', id: 'srvtoolu_1', name: 'web_search' },
          rawResult: null,
        },
      ]),
      // result 侧段（另一条密封 message）
      message(2, [
        {
          kind: 'webSearch',
          id: 'srvtoolu_1',
          query: '',
          results: [{ url: 'https://fx/1', title: '汇率来源', pageAge: null }],
          error: null,
          rawUse: null,
          rawResult: { type: 'web_search_tool_result', tool_use_id: 'srvtoolu_1' },
        },
      ]),
    ];

    const messages = fold(events, [recordRow('completed')]);
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    const assistants = screen
      .getAllByTestId('chat-bubble')
      .filter((bubble) => bubble.getAttribute('data-role') === 'assistant');
    expect(assistants).toHaveLength(1);
    const bubble = assistants[0] as HTMLElement;
    expect(within(bubble).getByTestId('block-web-search-query').textContent).toContain('今日汇率');
    // 来源列表在卡面展开后挂载（TraceCollapsible 默认折叠）
    fireEvent.click(within(within(bubble).getByTestId('block-web-search')).getByRole('button'));
    expect(within(bubble).getByTestId('block-web-search-sources').textContent).toContain(
      '汇率来源',
    );
  });
});

// ---------------------------------------------------------------------------
// 大列表分页（1000 条含空内容轮）
// ---------------------------------------------------------------------------

describe('AgentMessages：大列表分页', () => {
  beforeEach(() => {
    FakeIntersectionObserver.instances = [];
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('1000 条消息（含若干空内容轮）→ 默认渲染最新一页 10 条、remaining 正确、不抛错', () => {
    const normal = pagedConversation(1000);
    const empties = Array.from({ length: 50 }, (_, index) =>
      emptyTurnMessage(9000 + index, 'completed'),
    );

    render(<AgentMessages messages={[...normal, ...empties]} loading={false} running={false} />);

    // 空内容轮（completed）不计入过滤后列表：可见 1000 条，一页 10 条
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(10);
    expect(screen.getByTestId('history-load-hint').textContent).toContain('加载中…');
    // 触顶装载增一页（remaining 基于过滤后列表）
    triggerReachTop();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(20);
  });
});

// ---------------------------------------------------------------------------
// text 部件 citations 传递
// ---------------------------------------------------------------------------

describe('AgentMessages：text 部件 citations 传递', () => {
  it('带 metadata 的 text 部件渲染引用列表（title ?? url、按 url 去重），另一无 metadata 部件无列表', () => {
    const messages: AgentUIMessage[] = [
      {
        id: 'turn-9',
        role: 'assistant',
        metadata: { seq: 9, parentToolUseId: null, record: null },
        parts: [
          {
            type: 'text',
            text: '带引用正文',
            state: 'done',
            providerMetadata: {
              agent: {
                citations: [
                  { url: 'https://dup/1', title: '引用标题', citedText: null, raw: null },
                  { url: 'https://dup/1', title: '重复标题', citedText: null, raw: null },
                  { url: 'https://only/2', title: null, citedText: null, raw: null },
                ],
              },
            },
          },
          { type: 'text', text: '无引用正文', state: 'done' },
        ],
      },
    ];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    const lists = screen.getAllByTestId('block-citations');
    expect(lists).toHaveLength(1);
    const items = lists[0].querySelectorAll('li');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe('引用标题');
    expect(items[1].textContent).toBe('https://only/2');
    expect(screen.getAllByTestId('block-text')).toHaveLength(2);
  });

  it('metadata 宽解析边界：providerMetadata 缺席 / agent 缺席 / citations 非数组 / 空数组 → 一律按无引用处理', () => {
    const variants: unknown[] = [
      undefined,
      { other: {} },
      { agent: {} },
      { agent: { citations: { url: 'https://obj/1' } } },
      { agent: { citations: '字符串' } },
      { agent: { citations: null } },
      { agent: { citations: [] } },
    ];

    for (const providerMetadata of variants) {
      const messages: AgentUIMessage[] = [
        {
          id: 'turn-10',
          role: 'assistant',
          metadata: { seq: 10, parentToolUseId: null, record: null },
          parts: [
            {
              type: 'text',
              text: '正文',
              state: 'done',
              providerMetadata: providerMetadata as never,
            },
          ],
        },
      ];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(screen.queryByTestId('block-citations')).toBeNull();
      expect(screen.getByTestId('block-text')).toBeTruthy();
      unmount();
    }
  });
});
