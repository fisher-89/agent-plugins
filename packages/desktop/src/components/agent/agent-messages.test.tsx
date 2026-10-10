import { act, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import {
  eventsToUIMessages,
  runRecordToUIMessage,
  type AgentUIMessage,
} from '../../lib/agent-adapter';
import type { AgentBlock, AgentEvent, TurnSummary } from '../../types/dto';
import { AgentMessages } from './agent-messages';

// ---------------------------------------------------------------------------
// 纯渲染组件，消费方注入 AgentUIMessage[]（由适配层真实输出构造 fixture），
// 无进程边界，不需要 Mock。
// ---------------------------------------------------------------------------

const TS = 1727000000000;

function message(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
): AgentEvent {
  return { seq, timestampMs: TS, kind: 'message', role: 'assistant', blocks, parentToolUseId };
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

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function systemNotice(seq: number): AgentEvent {
  return { seq, timestampMs: TS, kind: 'systemNotice', subtype: 'permission_denial', payload: {} };
}

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery"}',
  };
}

function recordRow(status: TurnSummary['status']): TurnSummary {
  return {
    turnId: 1,
    sessionId: 'ses-0-1727000000000',
    status,
    startedAt: TS,
    finishedAt: TS + 999,
    numTurns: 2,
    costUsd: 0.2,
    durationMs: 800,
    error: null,
  };
}

/** 对话气泡基线：user 文本 + assistant 思考/正文 + 同事件工具对卡。 */
function bubbleEvents(): AgentEvent[] {
  return [
    userMessage(0, '帮我看看重试逻辑'),
    message(1, [
      { kind: 'thinking', thinking: '先梳理调用链' },
      { kind: 'text', text: '**重试**采用指数退避' },
    ]),
    message(2, [
      { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
      { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
    ]),
  ];
}

// ---------------------------------------------------------------------------
// 四变体气泡与注册表特化卡
// ---------------------------------------------------------------------------

describe('AgentMessages：四变体气泡', () => {
  it('user 文本、assistant 文本、reasoning（思考折叠）、tool 对卡四类部件分别呈现', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={false}
        running={false}
      />,
    );

    const bubbles = screen.getAllByTestId('chat-bubble');
    expect(bubbles).toHaveLength(3);
    expect(bubbles.map((bubble) => bubble.getAttribute('data-role'))).toEqual([
      'user',
      'assistant',
      'assistant',
    ]);

    const texts = screen.getAllByTestId('block-text');
    expect(texts).toHaveLength(2);
    expect(texts[0]?.textContent).toContain('帮我看看重试逻辑');
    expect(texts[1]?.textContent).toContain('重试');

    expect(screen.getByTestId('block-thinking').textContent).toContain('先梳理调用链');

    expect(screen.getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe('tu_1');
    expect(screen.getByTestId('block-tool-result').textContent).toContain('目录内容');
  });

  it('user 气泡靠右（align=end）、assistant 靠左（对齐属性迁移基线）', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={false}
        running={false}
      />,
    );

    const bubbles = screen.getAllByTestId('chat-bubble');
    expect(bubbles[0]?.getAttribute('data-align')).toBe('end');
    expect(bubbles[1]?.getAttribute('data-align')).toBe('start');
    // 内容块底色：user 泡 bg-muted、assistant 泡 bg-card
    const userContent = bubbles[0]?.querySelector('[data-slot="message-content"]');
    const assistantContent = bubbles[1]?.querySelector('[data-slot="message-content"]');
    expect(userContent?.className).toContain('bg-muted');
    expect(assistantContent?.className).toContain('bg-card');
  });

  it('AskUserQuestion 经注册表呈现静态卡：问题与选项结构化可读', () => {
    const events: AgentEvent[] = [
      message(0, [
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
    render(<AgentMessages messages={eventsToUIMessages(events)} loading={false} running={false} />);

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
// system 消息过滤（回溯修订：辅助行 / raw / record 不在对话透镜呈现）
// ---------------------------------------------------------------------------

describe('AgentMessages：system 消息过滤', () => {
  it('入参含 system 载体消息（run-started / system-notice / raw / record）→ 整条不进渲染列表', () => {
    const carrierMessages = [
      ...eventsToUIMessages([runStarted(10), systemNotice(11), raw(12)]),
      runRecordToUIMessage(recordRow('completed')),
    ];
    const bubbles = eventsToUIMessages(bubbleEvents());
    const messages = [...bubbles, ...carrierMessages];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    // 载体消息的 data 部件不出现在对话透镜
    expect(screen.queryByTestId('event-run-started')).toBeNull();
    expect(screen.queryByTestId('event-system')).toBeNull();
    expect(screen.queryByTestId('event-raw')).toBeNull();
    expect(screen.queryByTestId('event-run-record')).toBeNull();
    // user / assistant 气泡不受影响
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(3);
    // 过滤不回写入参组
    expect(messages).toHaveLength(bubbles.length + carrierMessages.length);
  });
});

// ---------------------------------------------------------------------------
// 容器组合与状态呈现
// ---------------------------------------------------------------------------

describe('AgentMessages：容器组合', () => {
  it('message-scroller 组合与滚动容器结构与迁移前一致（Provider + 视口 + 到底按钮）', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={false}
        running={false}
      />,
    );

    const section = screen.getByTestId('agent-messages');
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(section.className).toContain(className);
    }
    expect(document.querySelector('[data-slot="message-scroller"]') !== null).toBe(true);
    const viewport = document.querySelector('[data-slot="message-scroller-viewport"]');
    expect(viewport?.getAttribute('aria-label')).toBe('对话消息');
    expect(document.querySelector('[data-slot="message-scroller-button"]') !== null).toBe(true);
    // 每条气泡一个 Item（消息状态留应用层的组合基线）
    expect(document.querySelectorAll('[data-slot="message-scroller-item"]')).toHaveLength(3);
  });
});

describe('AgentMessages：空态与运行中态', () => {
  it('messages 为空 → empty 态呈现，不抛错', () => {
    render(<AgentMessages messages={[]} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryByTestId('chat-bubble')).toBeNull();
  });

  it('全为 system 载体消息（过滤后为空）→ empty 态呈现（判定基于过滤后列表）', () => {
    const messages = [
      ...eventsToUIMessages([runStarted(0), systemNotice(1)]),
      runRecordToUIMessage(recordRow('stopped')),
    ];
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryByTestId('chat-bubble')).toBeNull();
  });

  it('loading 且可见列表为空 → 会话还原中态', () => {
    render(<AgentMessages messages={[]} loading={true} running={false} />);

    expect(screen.getByTestId('agent-messages-loading') !== null).toBe(true);
  });

  it('loading 但已有可见气泡 → 不呈现还原中态，气泡照常呈现', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={true}
        running={false}
      />,
    );

    expect(screen.queryByTestId('agent-messages-loading')).toBeNull();
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(3);
  });

  it('running 且无终态 record → 头部运行中标记呈现', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={false}
        running={true}
      />,
    );

    expect(screen.getByTestId('agent-messages-running') !== null).toBe(true);
    expect(within(screen.getByTestId('agent-messages')).getByText('运行中…') !== null).toBe(true);
  });

  it('非运行态无运行中标记', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={false}
        running={false}
      />,
    );

    expect(screen.queryByTestId('agent-messages-running')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 历史分页：默认最新一页，触顶装载更早（prepend 后的滚动锚定由
// message-scroller 原语的 preserveScrollOnPrepend 承担，此处只测自研分页层）
// ---------------------------------------------------------------------------

/** 交替 user/assistant 的 N 条对话；seqOffset 区分会话（id = evt-<seq>） */
function pagedConversation(count: number, seqOffset = 0): AgentUIMessage[] {
  const events: AgentEvent[] = [];
  for (let i = 0; i < count; i++) {
    const seq = seqOffset + i;
    if (i % 2 === 0) {
      events.push(userMessage(seq, `user-msg-${i}`));
    } else {
      events.push(message(seq, [{ kind: 'text', text: `assistant-msg-${i}` }]));
    }
  }
  return eventsToUIMessages(events);
}

/** jsdom 无 IntersectionObserver：仅记录装配参数，触顶由 trigger 显式驱动 */
class FakeIntersectionObserver {
  static instances: FakeIntersectionObserver[] = [];

  constructor(
    private readonly callback: (entries: { isIntersecting: boolean }[]) => void,
    public readonly init?: { root?: Element | null; rootMargin?: string },
  ) {
    FakeIntersectionObserver.instances.push(this);
  }

  observe(): void {}

  unobserve(): void {}

  takeRecords(): never[] {
    return [];
  }

  disconnect(): void {
    FakeIntersectionObserver.instances = FakeIntersectionObserver.instances.filter(
      (io) => io !== this,
    );
  }

  trigger(isIntersecting: boolean): void {
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
    expect(joined).toContain('assistant-msg-15'); // 当前页最早（i=15）
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
      ...eventsToUIMessages([runStarted(90), systemNotice(91), raw(92)]),
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
