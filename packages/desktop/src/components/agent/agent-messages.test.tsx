// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import { eventsToUIMessages, runRecordToUIMessage } from '../../lib/agent-adapter';
import type { AgentBlock, AgentEvent, AgentRunRecord } from '../../types/dto';
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

function recordRow(status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id: 1,
    prompt: '对话一轮',
    cwd: 'C:\\demo\\alpha',
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: TS,
    finishedAt: TS + 999,
    numTurns: 2,
    costUsd: 0.2,
    durationMs: 800,
    sessionId: 's-1',
    error: null,
    source: 'explore',
    sourceRef: '7',
    parentRunId: null,
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

    expect(screen.getByTestId('conversation-loading') !== null).toBe(true);
  });

  it('loading 但已有可见气泡 → 不呈现还原中态，气泡照常呈现', () => {
    render(
      <AgentMessages
        messages={eventsToUIMessages(bubbleEvents())}
        loading={true}
        running={false}
      />,
    );

    expect(screen.queryByTestId('conversation-loading')).toBeNull();
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

    expect(screen.getByTestId('conversation-running') !== null).toBe(true);
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

    expect(screen.queryByTestId('conversation-running')).toBeNull();
  });
});
