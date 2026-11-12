import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type * as ToolCards from '@/components/agent/tool-cards';

import { foldSessionMessages, type AgentUIMessage } from '../../lib/agent-adapter';
import { filterConversation } from '../../lib/agent-virtual';
import type { AgentBlock, AgentEvent, TurnSummary } from '../../types/dto';
import { AgentMessages } from './agent-messages';

// ---------------------------------------------------------------------------
// 探针装置（保留真实实现，仅记录 props）：注册表键 `BUILD_IN_TOOL_CARDS
// ['AskUserQuestion']` 上包一层零行为探针，把每张 ask 特化卡收到的 `askAnswer`
// 引用 push 进 `cardSpy` 后原样渲染真实卡。
// `importOriginal` 保留全部真实实现（其余卡 / parseAsk 照旧）；探针挂注册表键而
// 非组件名——`AskUserQuestionCard` 不是导出面，为测试加导出会污染死代码扫描。
// ---------------------------------------------------------------------------

const { cardSpy } = vi.hoisted(() => ({
  cardSpy: { refs: [] as unknown[] },
}));

type ToolCardProps = React.ComponentProps<ToolCards.AgentToolCard>;
type AskAnswerActions = ToolCardProps['askAnswer'];

vi.mock('@/components/agent/tool-cards', async (importOriginal) => {
  const actual = await importOriginal<typeof ToolCards>();
  const Real = actual.BUILD_IN_TOOL_CARDS['AskUserQuestion'];
  function ProbedAskCard(props: ToolCardProps) {
    cardSpy.refs.push(props.askAnswer ?? null);
    return <Real {...props} />;
  }
  return {
    ...actual,
    BUILD_IN_TOOL_CARDS: { ...actual.BUILD_IN_TOOL_CARDS, AskUserQuestion: ProbedAskCard },
  };
});

// ---------------------------------------------------------------------------
// 纯渲染组件，消费方注入 AgentUIMessage[]（由适配层真实折叠输出构造 fixture），
// 无进程边界，不需要 Mock。呈现形状是「全量虚拟窗口 + user 提问 sticky 常驻」：
// user 气泡恒在吸附层（各一条），assistant 气泡只在可见切片内成节点；
// 纵向几何（gap / paddingStart / paddingEnd）全走虚拟器。
// jsdom 零布局 ⇒ 每项高度即 estimateSize（user 72 / assistant 240），
// 内容高度 = 12 + Σ估值 + 24 × (n − 1) + 12（AC-6 的账目判据）。
// ---------------------------------------------------------------------------

const TS = 1727000000000;
/** 虚拟器估值常量（与 agent-messages.tsx 的角色分型一致） */
const USER_ESTIMATE = 72;
const ASSISTANT_ESTIMATE = 240;
const GAP = 24;
const PADDING = 12;

function message(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
  turnId = seq + 1,
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

function userMessage(seq: number, text: string, turnId = seq + 1): AgentEvent {
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

function runStarted(seq: number, turnId = seq + 1): AgentEvent {
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

function systemNotice(seq: number, turnId = seq + 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: TS,
    kind: 'systemNotice',
    subtype: 'permission_denial',
    payload: {},
  };
}

function raw(seq: number, turnId = seq + 1): AgentEvent {
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
  turnId = 1,
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

/** 对话气泡基线：单轮（提问 + 思考/正文 + 工具对卡）→ 两条气泡（user 在吸附层） */
function bubbleEvents(): AgentEvent[] {
  return [
    userMessage(0, '帮我看看重试逻辑', 1),
    message(
      1,
      [
        { kind: 'thinking', thinking: '先梳理调用链' },
        { kind: 'text', text: '**重试**采用指数退避' },
        { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        { kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false },
      ],
      null,
      1,
    ),
  ];
}

/** 交替 user / assistant 的 N 条多轮会话（每对一轮，轮行按 completed 给） */
function turnConversation(count: number, seqOffset = 0): AgentUIMessage[] {
  const events: AgentEvent[] = [];
  const turns: TurnSummary[] = [];
  for (let index = 0; index < count; index += 2) {
    const turnId = seqOffset + index + 1;
    turns.push(recordRow('completed', turnId));
    events.push(userMessage(seqOffset + index, `user-msg-${index}`, turnId));
    if (index + 1 < count) {
      events.push(
        message(
          seqOffset + index + 1,
          [{ kind: 'text', text: `assistant-msg-${index}` }],
          null,
          turnId,
        ),
      );
    }
  }
  return fold(events, turns);
}

/** 交替角色序列（与 `turnConversation` 的产出同形：user 起、偶数位 user） */
function alternatingRoles(count: number): ('user' | 'assistant')[] {
  return Array.from({ length: count }, (_, index) => (index % 2 === 0 ? 'user' : 'assistant'));
}

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

function bubblesOfRole(role: 'user' | 'assistant'): HTMLElement[] {
  return screen
    .queryAllByTestId('chat-bubble')
    .filter((bubble) => bubble.getAttribute('data-role') === role);
}

function itemNodes(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[data-slot="message-scroller-item"]')];
}

function stickyNodes(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[data-slot="message-scroller-sticky"]')];
}

function contentNode(): HTMLElement {
  const node = document.querySelector<HTMLElement>('[data-slot="message-scroller-content"]');
  if (node === null) throw new Error('缺少 message-scroller-content');
  return node;
}

function contentHeight(): number {
  return Number.parseFloat(contentNode().style.height);
}

/** 估值账目：12 + Σ估值 + 24 × (n − 1) + 12（jsdom 零测量 ⇒ 估值即几何） */
function expectedHeight(roles: ('user' | 'assistant')[]): number {
  const sizes = roles.map((role) => (role === 'user' ? USER_ESTIMATE : ASSISTANT_ESTIMATE));
  const sum = sizes.reduce((acc, size) => acc + size, 0);
  return PADDING * 2 + sum + GAP * Math.max(roles.length - 1, 0);
}

/** 估值起点序列（与虚拟项 translateY 同基准） */
function expectedStarts(roles: ('user' | 'assistant')[]): number[] {
  const starts: number[] = [];
  let cursor = PADDING;
  for (const role of roles) {
    starts.push(cursor);
    cursor += (role === 'user' ? USER_ESTIMATE : ASSISTANT_ESTIMATE) + GAP;
  }
  return starts;
}

function translateYOf(node: HTMLElement): number {
  const matched = /translateY\((-?[\d.]+)px\)/.exec(node.style.transform);
  if (matched === null) throw new Error(`缺少 translateY: ${node.style.transform}`);
  return Number(matched[1]);
}

type ResizeEntries = { borderBoxSize: { blockSize: number }[] }[];

/** 可编程 ResizeObserver 替身：由用例手动投 borderBoxSize（test-setup 的零回调桩为缺省值） */
class ProgrammableResizeObserver {
  public static instances: ProgrammableResizeObserver[] = [];

  public readonly targets: Element[] = [];

  constructor(private readonly callback: (entries: ResizeEntries) => void) {
    ProgrammableResizeObserver.instances.push(this);
  }

  public observe(target: Element): void {
    this.targets.push(target);
  }

  public unobserve(): void {}

  public disconnect(): void {}

  public trigger(blockSize: number): void {
    this.callback([{ borderBoxSize: [{ blockSize }] }]);
  }
}

function stickyObserver(): ProgrammableResizeObserver | undefined {
  return ProgrammableResizeObserver.instances.find((instance) =>
    instance.targets.some(
      (target) => target.closest('[data-slot="message-scroller-sticky"]') !== null,
    ),
  );
}

/** 实例级滚动写入桩：jsdom 的 `Element` 无 `scrollTo`、`scrollTop` 不写回，
 * 故按实例装最小桩（只写 `scrollTop`、不派发 `scroll` 事件），并把
 * `scrollHeight` 抬到「内容一屏以上」使 `maxScroll > 0`。 */
function installScrollStub(): { top: () => number; setTop: (value: number) => void } {
  const viewport = document.querySelector<HTMLElement>('[data-slot="message-scroller-viewport"]');
  if (viewport === null) throw new Error('缺少视口');
  let top = 0;
  Object.defineProperty(viewport, 'scrollHeight', {
    configurable: true,
    value: contentHeight() + 600,
  });
  Object.defineProperty(viewport, 'scrollTop', {
    configurable: true,
    get: () => top,
    set: (value: number) => {
      top = value;
    },
  });
  Object.defineProperty(viewport, 'scrollTo', {
    configurable: true,
    value: (options: { top?: number }) => {
      top = options.top ?? 0;
    },
  });
  return {
    top: () => top,
    setTop: (value: number) => {
      top = value;
    },
  };
}

/** 贴底目标值：`scrollHeight = 内容高度 + 视口高` ⇒ `maxScroll = 内容高度`，
 * 末项 `align:'end'` 扣 `paddingEnd` */
function endTarget(): number {
  return Math.max(contentHeight() - PADDING, 0);
}

/** 会话身份可指定的一轮对话（user + assistant，轮行带该会话 id） */
function sessionConversation(sessionId: string): AgentUIMessage[] {
  return fold(
    [userMessage(0, '问', 1), message(1, [{ kind: 'text', text: '答' }], null, 1)],
    [{ ...recordRow('completed', 1), sessionId }],
  );
}

afterEach(() => {
  ProgrammableResizeObserver.instances = [];
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 四变体气泡与注册表特化卡
// ---------------------------------------------------------------------------

describe('AgentMessages：四变体气泡', () => {
  it('user 文本、assistant 文本、reasoning（思考折叠）、tool 对卡四类部件分别呈现', async () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(bubblesOfRole('assistant')).toHaveLength(1);

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

    const [userBubble] = bubblesOfRole('user');
    const [assistantBubble] = bubblesOfRole('assistant');
    // user 泡改整宽表达，不再靠 align 属性撑右对齐
    expect(userBubble?.getAttribute('data-align')).toBe('start');
    expect(assistantBubble?.getAttribute('data-align')).toBe('start');
    // 内容块底色：user 泡走 Bubble tinted，assistant 泡无泡底
    const tinted = userBubble?.querySelector('[data-slot="bubble"]');
    expect(tinted?.getAttribute('data-variant')).toBe('tinted');
    expect(tinted?.querySelector('[data-slot="bubble-content"]')).toBeTruthy();
    expect(assistantBubble?.querySelector('[data-slot="bubble"]')).toBeNull();
  });

  it('一轮一条气泡：同轮两次响应（多段密封块）合成单个 assistant 气泡', () => {
    const events: AgentEvent[] = [
      userMessage(0, '问', 1),
      message(1, [{ kind: 'text', text: '第一段' }], null, 1),
      message(2, [{ kind: 'text', text: '第二段' }], null, 1),
    ];

    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    const assistants = bubblesOfRole('assistant');
    expect(assistants).toHaveLength(1);
    const assistantTexts = within(assistants[0] as HTMLElement)
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
// 超长 user 提问默认折叠（短提问不折叠、展开回到原渲染件、多部件不折叠）
// ---------------------------------------------------------------------------

describe('AgentMessages：超长 user 提问默认折叠', () => {
  it('短提问（一二行）→ 无折叠触发按钮，正文直出', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    const [userBubble] = bubblesOfRole('user');
    expect(within(userBubble as HTMLElement).queryByTestId('user-bubble-toggle')).toBeNull();
    expect(within(userBubble as HTMLElement).queryByTestId('user-bubble-preview')).toBeNull();
    expect(screen.getAllByTestId('block-text')[0]?.textContent).toContain('帮我看看重试逻辑');
  });

  it('超长提问 → 折叠为预览 + 「展开」；点开呈全文与「收起」，再点回预览', () => {
    const long = Array.from({ length: 6 }, (_, index) => `第 ${index + 1} 行提问内容`).join('\n');
    const events: AgentEvent[] = [
      userMessage(0, long, 1),
      message(1, [{ kind: 'text', text: '答' }], null, 1),
    ];
    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    const [userBubble] = bubblesOfRole('user');
    expect(within(userBubble as HTMLElement).queryByTestId('user-bubble-preview')).not.toBeNull();
    const toggle = within(userBubble as HTMLElement).getByTestId('user-bubble-toggle');
    expect(toggle.textContent).toContain('展开');
    expect(toggle.hasAttribute('data-panel-open')).toBe(false);

    fireEvent.click(toggle);
    expect(within(userBubble as HTMLElement).queryByTestId('user-bubble-preview')).toBeNull();
    // 展开态经 base-ui 的 `data-panel-open` 驱动箭头翻转（CSS 判据不在 jsdom 求值）
    expect(
      within(userBubble as HTMLElement)
        .getByTestId('user-bubble-toggle')
        .hasAttribute('data-panel-open'),
    ).toBe(true);
    expect(within(userBubble as HTMLElement).getByTestId('block-text').textContent).toContain(
      '第 6 行提问内容',
    );
    expect(
      within(userBubble as HTMLElement).getByTestId('user-bubble-toggle').textContent,
    ).toContain('收起');

    fireEvent.click(within(userBubble as HTMLElement).getByTestId('user-bubble-toggle'));
    expect(within(userBubble as HTMLElement).queryByTestId('user-bubble-preview')).not.toBeNull();
    expect(within(userBubble as HTMLElement).queryByTestId('block-text')).toBeNull();
  });

  it('折叠预览保留前若干行并以省略号收尾（内容确实被截短）', () => {
    const long = Array.from({ length: 8 }, (_, index) => `line-${index + 1}`).join('\n');
    const events: AgentEvent[] = [
      userMessage(0, long, 1),
      message(1, [{ kind: 'text', text: '答' }], null, 1),
    ];
    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    const preview = screen.getByTestId('user-bubble-preview').textContent ?? '';
    expect(preview.endsWith('…')).toBe(true);
    expect(preview).toContain('line-1');
    expect(preview).not.toContain('line-8');
  });

  it('含非 text 部件的 user 消息不参与折叠（预览与按钮缺席，部件原样呈现）', () => {
    const message: AgentUIMessage = {
      id: 'turn-1-user',
      role: 'user',
      metadata: { seq: 0, parentToolUseId: null },
      parts: [
        { type: 'text', text: Array.from({ length: 8 }, () => '很长的一行').join('\n') },
        { type: 'reasoning', text: '附带思考', state: 'done' },
      ],
    };

    render(<AgentMessages messages={[message]} loading={false} running={false} />);

    expect(screen.queryByTestId('user-bubble-toggle')).toBeNull();
    expect(screen.queryByTestId('user-bubble-preview')).toBeNull();
    expect(screen.getByTestId('block-text')).toBeTruthy();
    expect(screen.getByTestId('block-thinking')).toBeTruthy();
  });

  it('折行估算：单行长文本（无换行）超过行高预算同样折叠', () => {
    const events: AgentEvent[] = [
      userMessage(0, '一'.repeat(200), 1),
      message(1, [{ kind: 'text', text: '答' }], null, 1),
    ];
    render(<AgentMessages messages={fold(events)} loading={false} running={false} />);

    expect(screen.getByTestId('user-bubble-preview')).not.toBeNull();
    expect(screen.getByTestId('user-bubble-toggle').textContent).toContain('展开');
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
    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
    // 过滤不回写入参组
    expect(messages.length).toBeGreaterThan(bubbles.length);
  });
});

// ---------------------------------------------------------------------------
// 容器组合与状态呈现（虚拟化形态）
// ---------------------------------------------------------------------------

describe('AgentMessages：容器组合', () => {
  it('message-scroller 组合与滚动容器结构：Provider + 视口 + 内容 + 到底按钮', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={false} running={false} />);

    const section = screen.getByTestId('agent-messages');
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(section.className).toContain(className);
    }
    expect(document.querySelector('[data-slot="message-scroller"]') !== null).toBe(true);
    const viewport = document.querySelector('[data-slot="message-scroller-viewport"]');
    expect(viewport?.getAttribute('aria-label')).toBe('对话消息');
    expect(document.querySelector('[data-slot="message-scroller-button"]') !== null).toBe(true);
    // 虚拟窗口：切片项数由窗口决定（> 0）且不含 user 索引（user 恒在吸附层）
    const items = itemNodes();
    expect(items.length).toBeGreaterThan(0);
    expect(items.every((node) => node.querySelector('[data-role="user"]') === null)).toBe(true);
    expect(items.map((node) => node.getAttribute('data-index'))).toEqual(['1']);
  });
});

describe('AgentMessages：空态与运行中态', () => {
  it('messages 为空 → empty 态呈现，不抛错', () => {
    render(<AgentMessages messages={[]} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(0);
  });

  it('全为 system 载体消息（过滤后为空）→ empty 态呈现（判定基于过滤后列表）', () => {
    const messages = fold([runStarted(0, 1), systemNotice(1, 2)], []);
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(0);
  });

  it('loading 且可见列表为空 → 会话还原中态', () => {
    render(<AgentMessages messages={[]} loading={true} running={false} />);

    expect(screen.getByTestId('agent-messages-loading') !== null).toBe(true);
  });

  it('loading 但已有可见气泡 → 不呈现还原中态，气泡照常呈现', () => {
    render(<AgentMessages messages={fold(bubbleEvents())} loading={true} running={false} />);

    expect(screen.queryByTestId('agent-messages-loading')).toBeNull();
    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
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
// 全量虚拟窗口（AC-1 / AC-6）：只渲可见切片，账目按全量推算
// ---------------------------------------------------------------------------

describe('AgentMessages：全量虚拟窗口', () => {
  it('1000 条可见消息 + 50 条空内容 completed 轮 → assistant 气泡 ≪ 1000、user 全量常驻', () => {
    const normal = turnConversation(1000);
    const empties = Array.from({ length: 50 }, (_, index) =>
      emptyTurnMessage(9000 + index, 'completed'),
    );

    render(<AgentMessages messages={[...normal, ...empties]} loading={false} running={false} />);

    const assistants = bubblesOfRole('assistant');
    expect(assistants.length).toBeGreaterThan(0);
    expect(assistants.length).toBeLessThan(100);
    expect(bubblesOfRole('user')).toHaveLength(500);
    // 空内容轮（completed）不计入过滤后列表：账目按 1000 条推算
    expect(contentHeight()).toBe(expectedHeight(alternatingRoles(1000)));
    // 分页面退役：零提示行、零触顶观察器装配
    expect(document.querySelectorAll('[data-testid^="history-load"]')).toHaveLength(0);
  });

  it('同一 1000 条 fixture → 末项底边与内容底边对齐（末项 translateY + 估值 + paddingEnd = height）', () => {
    render(<AgentMessages messages={turnConversation(1000)} loading={false} running={false} />);
    const viewport = screen
      .getByTestId('agent-messages')
      .querySelector<HTMLElement>('[data-slot="message-scroller-viewport"]');
    const height = contentHeight();
    expect(height).toBe(expectedHeight(alternatingRoles(1000)));

    // 滚到末端：实例级抬高 scrollHeight / scrollTop（原型桩 600/600 恒贴底、maxScroll = 0）
    act(() => {
      Object.defineProperty(viewport, 'scrollHeight', {
        configurable: true,
        value: height + 600,
      });
      Object.defineProperty(viewport, 'scrollTop', {
        configurable: true,
        value: height,
        writable: true,
      });
      fireEvent.scroll(viewport!);
    });

    const last = itemNodes().at(-1)!;
    expect(last.getAttribute('data-index')).toBe('999');
    expect(translateYOf(last) + ASSISTANT_ESTIMATE + PADDING).toBe(height);
  });
});

// ---------------------------------------------------------------------------
// user 消息常驻吸附（AC-3 / AC-4）：吸附层几何与虚拟层零 user 节点
// ---------------------------------------------------------------------------

describe('AgentMessages：user 消息常驻吸附', () => {
  it('12 条 user → 吸附层恰 12 条（各一条 user 气泡），虚拟层 user 节点为零', () => {
    render(<AgentMessages messages={turnConversation(24)} loading={false} running={false} />);

    const stickies = stickyNodes();
    expect(stickies).toHaveLength(12);
    expect(bubblesOfRole('user')).toHaveLength(12);
    expect(stickies.every((node) => node.querySelectorAll('[data-role="user"]').length === 1)).toBe(
      true,
    );
    const items = itemNodes();
    expect(items.length).toBeGreaterThan(0);
    expect(items.every((node) => node.querySelector('[data-role="user"]') === null)).toBe(true);
  });

  it('吸附层几何与虚拟项同基准：sticky top 逐条等于该索引的估值起点，且无 transform', () => {
    render(<AgentMessages messages={turnConversation(24)} loading={false} running={false} />);
    const roles = alternatingRoles(24);
    const starts = expectedStarts(roles);

    stickyNodes().forEach((node, position) => {
      const index = position * 2; // user 索引 0 / 2 / 4 …
      expect(Number.parseFloat(node.style.top), `sticky-${position}`).toBe(starts[index]);
      expect(node.style.getPropertyValue('transform'), `sticky-${position}`).toBe('');
      expect(node.getAttribute('style')?.toLowerCase()).not.toContain('transform');
    });
  });

  it('吸附层 span 首尾闭合：第 k 条 height = 第 k+1 条 top − 第 k 条 top，末条闭合到内容高度', () => {
    render(<AgentMessages messages={turnConversation(24)} loading={false} running={false} />);
    const stickies = stickyNodes();
    const tops = stickies.map((node) => Number.parseFloat(node.style.top));
    const heights = stickies.map((node) => Number.parseFloat(node.style.height));
    const total = contentHeight();

    for (let index = 0; index < stickies.length - 1; index += 1) {
      expect(heights[index], `span-${index}`).toBe((tops[index + 1] ?? 0) - (tops[index] ?? 0));
    }
    expect(heights.at(-1)).toBe(total - (tops.at(-1) ?? 0));
    expect(Math.min(...heights)).toBeGreaterThanOrEqual(0);
  });

  it('气泡同渲染件：吸附层 user 气泡为 tinted 泡 + 底衬容器，零渐变 / 零阴影', () => {
    render(<AgentMessages messages={turnConversation(4)} loading={false} running={false} />);
    const sticky = stickyNodes()[0] as HTMLElement;
    const inner = sticky.firstElementChild as HTMLElement;

    const bubble = sticky.querySelector('[data-testid="chat-bubble"]');
    expect(bubble?.getAttribute('data-role')).toBe('user');
    expect(bubble?.querySelector('[data-slot="bubble"]')?.getAttribute('data-variant')).toBe(
      'tinted',
    );
    expect(inner.className).toContain('bg-card');
    for (const forbidden of ['shadow', 'bg-gradient', 'from-', 'to-']) {
      expect(inner.className, forbidden).not.toContain(forbidden);
    }
  });

  it('异常：无 user 消息（全 assistant）→ 零吸附层节点，不抛错', () => {
    const messages = fold([message(1, [{ kind: 'text', text: '只有回答' }])]);

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(stickyNodes()).toHaveLength(0);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
  });

  it('边界：单条 user → 恰 1 条 sticky，top 为 paddingStart、height 为内容高度减之', () => {
    render(<AgentMessages messages={turnConversation(1)} loading={false} running={false} />);
    const sticky = stickyNodes()[0] as HTMLElement;
    const total = contentHeight();

    expect(stickyNodes()).toHaveLength(1);
    expect(Number.parseFloat(sticky.style.top)).toBe(PADDING);
    expect(Number.parseFloat(sticky.style.height)).toBe(total - PADDING);
    expect(bubblesOfRole('assistant')).toHaveLength(0);
  });

  it('边界：单条 assistant（无 user）→ 零 sticky、恰 1 条 assistant 气泡', () => {
    const messages = fold([message(1, [{ kind: 'text', text: '只有回答' }])]);

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(stickyNodes()).toHaveLength(0);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
  });

  it('异常：过滤后为空（空列表 / 全 system）→ 零 sticky、零 item、空态块仍在', () => {
    const cases: [string, AgentUIMessage[]][] = [
      ['空列表', []],
      ['全 system', fold([runStarted(0, 1), systemNotice(1, 2)], [])],
    ];

    for (const [label, messages] of cases) {
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );
      expect(stickyNodes(), label).toHaveLength(0);
      expect(itemNodes(), label).toHaveLength(0);
      expect(screen.getByTestId('conversation-empty'), label).not.toBeNull();
      expect(contentHeight(), label).toBe(PADDING * 2);
      unmount();
    }
  });

  it('异常：吸附层高度回填 —— 实测 0 回落 estimateSize、实测 88 生效且账目随之变化', () => {
    vi.stubGlobal('ResizeObserver', ProgrammableResizeObserver);
    ProgrammableResizeObserver.instances = [];
    render(<AgentMessages messages={turnConversation(2)} loading={false} running={false} />);
    const roles = alternatingRoles(2);
    const baseHeight = expectedHeight(roles);
    expect(contentHeight()).toBe(baseHeight);

    const observer = stickyObserver();
    expect(observer).toBeDefined();

    act(() => {
      observer?.trigger(0);
    });
    expect(contentHeight()).toBe(baseHeight);
    expect(Number.parseFloat((stickyNodes()[0] as HTMLElement).style.height)).toBe(
      baseHeight - PADDING,
    );

    act(() => {
      observer?.trigger(88);
    });
    expect(contentHeight()).toBe(baseHeight - USER_ESTIMATE + 88);
    expect(Number.parseFloat((stickyNodes()[0] as HTMLElement).style.height)).toBe(
      baseHeight - USER_ESTIMATE + 88 - PADDING,
    );
  });

  it('异常：反向判据 —— 零 history-load-* 节点、零 IntersectionObserver 装配、Content 无 p-3', () => {
    class CountingObserver {
      public static instances = 0;

      constructor() {
        CountingObserver.instances += 1;
      }

      public observe(): void {}

      public unobserve(): void {}

      public disconnect(): void {}
    }
    vi.stubGlobal('IntersectionObserver', CountingObserver);

    const { container } = render(
      <AgentMessages messages={turnConversation(30)} loading={false} running={false} />,
    );

    expect(CountingObserver.instances).toBe(0);
    expect(screen.queryByTestId('history-load-hint')).toBeNull();
    expect(screen.queryByTestId('history-load-complete')).toBeNull();
    expect(container.querySelectorAll('[data-testid^="history-load"]')).toHaveLength(0);
    // 触顶 sentinel 产物（h-2 w-full）缺席
    expect(contentNode().querySelector('.h-2')).toBeNull();
    expect(contentNode().className).not.toContain('p-3');
  });

  it('边界：estimateSize 角色分型 —— 全 user 与全 assistant（同条数）高度差为 168 × n', () => {
    const build = (count: number, role: 'user' | 'assistant'): AgentUIMessage[] => {
      const events: AgentEvent[] = [];
      const turns: TurnSummary[] = [];
      for (let index = 0; index < count; index += 1) {
        turns.push(recordRow('completed', index + 1));
        events.push(
          role === 'user'
            ? userMessage(index, `问-${index}`, index + 1)
            : message(index, [{ kind: 'text', text: `答-${index}` }], null, index + 1),
        );
      }
      return fold(events, turns);
    };

    const usersFixture = build(6, 'user');
    const assistantsFixture = build(6, 'assistant');
    expect(filterConversation(usersFixture)).toHaveLength(6);
    expect(filterConversation(assistantsFixture)).toHaveLength(6);

    const users = render(<AgentMessages messages={usersFixture} loading={false} running={false} />);
    const userHeight = contentHeight();
    users.unmount();

    const assistants = render(
      <AgentMessages messages={assistantsFixture} loading={false} running={false} />,
    );
    const assistantHeight = contentHeight();
    assistants.unmount();

    expect(assistantHeight - userHeight).toBe((ASSISTANT_ESTIMATE - USER_ESTIMATE) * 6);
  });
});

// ---------------------------------------------------------------------------
// 贴底复位（AC-13）：换会话回末端，同会话追加与键单侧为 null 一概不拽回
// ---------------------------------------------------------------------------

describe('AgentMessages：首帧与换会话贴底复位', () => {
  it('正向：换会话（sessionId A → B、条数不变）→ 视口复位回末端目标值', () => {
    const { rerender } = render(
      <AgentMessages messages={sessionConversation('A')} loading={false} running={false} />,
    );
    const scroll = installScrollStub();

    act(() => {
      scroll.setTop(0);
    });
    rerender(<AgentMessages messages={sessionConversation('B')} loading={false} running={false} />);

    // 复位键经透镜重渲染通道成立：两侧非 null 且不等 ⇒ 贴底一次
    expect(scroll.top()).toBe(endTarget());
  });

  it('边界：只传 messages（loading / running 缺席）→ 复位键派生不改变入参契约', () => {
    const { rerender } = render(<AgentMessages messages={sessionConversation('A')} />);
    const scroll = installScrollStub();

    act(() => {
      scroll.setTop(0);
    });
    // 三字段外的额外入参一律不需要：复位完全由消息集合的会话身份驱动
    rerender(<AgentMessages messages={sessionConversation('B')} />);

    expect(scroll.top()).toBe(endTarget());
  });

  it('异常：同会话追加（sessionId 不变、尾部新增消息）→ 不把视口拽回末端', () => {
    const base = sessionConversation('A');
    const { rerender } = render(<AgentMessages messages={base} loading={false} running={false} />);
    const scroll = installScrollStub();

    act(() => {
      scroll.setTop(0);
    });
    rerender(
      <AgentMessages
        messages={[
          ...base,
          ...fold(
            [userMessage(2, '追问', 2), message(3, [{ kind: 'text', text: '追答' }], null, 2)],
            [{ ...recordRow('completed', 2), sessionId: 'A' }],
          ),
        ]}
        loading={false}
        running={false}
      />,
    );

    // 上滑用户不被拽走：端锚定可作保持补偿，但不得落到末端
    expect(scroll.top()).not.toBe(endTarget());
  });

  it('异常：复位键单侧为 null（首轮 record 落库、条数不变）→ 不复位', () => {
    const before = fold([message(1, [{ kind: 'text', text: '答' }])]);
    // 首轮未收口：轮行缺席 ⇒ conversationIdentity 返回 null
    expect(before[0]?.metadata?.record ?? null).toBeNull();

    const { rerender } = render(
      <AgentMessages messages={before} loading={false} running={false} />,
    );
    const scroll = installScrollStub();

    act(() => {
      scroll.setTop(17);
    });
    rerender(
      <AgentMessages
        messages={fold([message(1, [{ kind: 'text', text: '答' }])], [recordRow('completed', 1)])}
        loading={false}
        running={false}
      />,
    );

    expect(scroll.top()).toBe(17);
  });
});

// ---------------------------------------------------------------------------
// 空内容轮的可见性判据：按 metadata.record.status 判定
// ---------------------------------------------------------------------------

describe('AgentMessages：空内容轮可见性判据', () => {
  it('1 条空 parts + completed 轮 + 3 条正常消息 → 空气泡零渲染、正常气泡按角色分查', () => {
    const messages: AgentUIMessage[] = [
      ...fold(
        [userMessage(1, '第一问', 1), message(2, [{ kind: 'text', text: '第一答' }], null, 1)],
        [recordRow('completed', 1)],
      ),
      ...fold(
        [message(3, [{ kind: 'text', text: '第二答' }], null, 2)],
        [recordRow('completed', 2)],
      ),
      emptyTurnMessage(42, 'completed'),
    ];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(bubblesOfRole('assistant')).toHaveLength(2);
    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(document.body.textContent).toContain('第一问');
    expect(document.body.textContent).toContain('第一答');
    // 空内容 completed 轮不计入账目：3 条可见
    expect(contentHeight()).toBe(expectedHeight(['user', 'assistant', 'assistant']));
  });

  it('空内容轮批（50 条 completed）混入 1000 条正常对话 → 项数按 1000、气泡与账目同口径', () => {
    const messages = [
      ...turnConversation(1000),
      ...Array.from({ length: 50 }, (_, index) => emptyTurnMessage(5000 + index, 'completed')),
    ];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(contentHeight()).toBe(expectedHeight(alternatingRoles(1000)));
    expect(bubblesOfRole('user')).toHaveLength(500);
    expect(stickyNodes()).toHaveLength(500);
  });

  it('空内容 failed / stopped 轮计入虚拟列表项数与吸附层几何', () => {
    for (const status of ['failed', 'stopped'] as const) {
      const messages = [
        ...turnConversation(2),
        emptyTurnMessage(status === 'failed' ? 7 : 8, status),
      ];
      const { unmount } = render(
        <AgentMessages messages={messages} loading={false} running={false} />,
      );

      // 状态行载体在透镜内 ⇒ 计入 count 与账目高度
      expect(screen.getByTestId('turn-status-line').getAttribute('data-status'), status).toBe(
        status,
      );
      expect(contentHeight(), status).toBe(expectedHeight(['user', 'assistant', 'assistant']));
      const sticky = stickyNodes()[0] as HTMLElement;
      expect(Number.parseFloat(sticky.style.height), status).toBe(contentHeight() - PADDING);
      unmount();
    }
  });

  it('只含被消费孤儿搜索 result 的消息（折叠后 0 部件、record 为 null）不进对话透镜', () => {
    const events: AgentEvent[] = [
      userMessage(1, '帮我搜', 1),
      message(
        2,
        [
          {
            kind: 'webSearch',
            id: 'srvtoolu_ghost',
            query: '查询',
            results: [],
            error: null,
            rawUse: { type: 'server_tool_use', id: 'srvtoolu_ghost' },
            rawResult: null,
          },
        ],
        null,
        1,
      ),
      // 第二段：仅被 use 消费的 result 侧块 → 折叠后 parts.length === 0
      message(
        3,
        [
          {
            kind: 'webSearch',
            id: 'srvtoolu_ghost',
            query: '',
            results: [{ url: 'https://ghost/1', title: '幽灵来源', pageAge: null }],
            error: null,
            rawUse: null,
            rawResult: { type: 'web_search_tool_result', tool_use_id: 'srvtoolu_ghost' },
          },
        ],
        null,
        2,
      ),
    ];
    // 无轮行 ⇒ record 为 null；该轮 assistant 有搜索卡一个部件，故仍在透镜内
    const messages = fold(events);
    expect(messages.find((message) => message.role === 'assistant')?.parts).toHaveLength(1);

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// 轮内状态行：failed / stopped 空内容轮
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
      [userMessage(0, '问', 1), message(1, [{ kind: 'text', text: '部分回答' }], null, 1)],
      [recordRow('stopped', 1)],
    );

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.queryByTestId('turn-status-line')).toBeNull();
    expect(bubblesOfRole('user')).toHaveLength(1);
    expect(bubblesOfRole('assistant')).toHaveLength(1);
  });

  it('parts 非空 + failed → 只呈正常气泡，状态行缺席（轮失败事实仍可从 metadata 读出）', () => {
    const messages = fold(
      [message(1, [{ kind: 'text', text: '部分回答' }], null, 1)],
      [recordRow('failed', 1, 'provider 认证失败')],
    );

    expect(messages[0]?.metadata?.record?.status).toBe('failed');
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    expect(screen.queryByTestId('turn-status-line')).toBeNull();
    expect(bubblesOfRole('assistant')).toHaveLength(1);
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

  it('可见性判据单一来源：空 parts + failed 既计入虚拟列表账目又只渲染一次状态行', () => {
    const messages = [...turnConversation(4), emptyTurnMessage(700, 'failed')];

    render(<AgentMessages messages={messages} loading={false} running={false} />);

    // 过滤保留 ⇒ 计入 count 与账目高度
    expect(contentHeight()).toBe(
      expectedHeight(['user', 'assistant', 'user', 'assistant', 'assistant']),
    );
    // 渲染恰一次（不出现两处各渲染一次）
    expect(screen.queryAllByTestId('turn-status-line')).toHaveLength(1);
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
      expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(0);
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
      expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(0);
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// 一轮一条气泡（呈现侧）与 pause_turn 搜索两段合轮
// ---------------------------------------------------------------------------

describe('AgentMessages：一轮一条气泡与搜索两段合轮', () => {
  it('同一轮 text + reasoning + tool 三部件 → 恰 1 个 assistant 气泡，部件按序呈现', () => {
    const events: AgentEvent[] = [
      userMessage(0, '一轮多部件', 1),
      message(
        1,
        [
          { kind: 'thinking', thinking: '先想' },
          { kind: 'text', text: '再答' },
          { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        ],
        null,
        1,
      ),
    ];

    render(
      <AgentMessages
        messages={fold(events, [recordRow('completed', 1)])}
        loading={false}
        running={false}
      />,
    );

    const assistants = bubblesOfRole('assistant');
    expect(assistants).toHaveLength(1);
    const bubble = assistants[0] as HTMLElement;
    expect(within(bubble).getByTestId('block-thinking')).toBeTruthy();
    expect(within(bubble).getByTestId('block-text').textContent).toBe('再答');
    expect(within(bubble).getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe('tu_1');
  });

  it('pause_turn 两段同轮同消息：同一气泡内搜索卡呈查询词与来源列表，无第二条气泡', () => {
    const events: AgentEvent[] = [
      userMessage(0, '帮我查今日汇率', 1),
      // use 侧段
      message(
        1,
        [
          {
            kind: 'webSearch',
            id: 'srvtoolu_1',
            query: '今日汇率',
            results: [],
            error: null,
            rawUse: { type: 'server_tool_use', id: 'srvtoolu_1', name: 'web_search' },
            rawResult: null,
          },
        ],
        null,
        1,
      ),
      // result 侧段（另一条密封 message）
      message(
        2,
        [
          {
            kind: 'webSearch',
            id: 'srvtoolu_1',
            query: '',
            results: [{ url: 'https://fx/1', title: '汇率来源', pageAge: null }],
            error: null,
            rawUse: null,
            rawResult: { type: 'web_search_tool_result', tool_use_id: 'srvtoolu_1' },
          },
        ],
        null,
        1,
      ),
    ];

    const messages = fold(events, [recordRow('completed', 1)]);
    render(<AgentMessages messages={messages} loading={false} running={false} />);

    const assistants = bubblesOfRole('assistant');
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
    const items = lists[0]?.querySelectorAll('li') ?? [];
    expect(items).toHaveLength(2);
    expect(items[0]?.textContent).toBe('引用标题');
    expect(items[1]?.textContent).toBe('https://only/2');
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

// ---------------------------------------------------------------------------
// ask-tool-in-flow：ask 卡三态判活矩阵（`pendingToolCallId`）与引用稳定
// ---------------------------------------------------------------------------

/** 一轮对话含两条 ask 工具调用（`call_1` / `call_2`，均为无结果态）。 */
function twoAskEvents(): AgentEvent[] {
  return [
    userMessage(0, '请裁决', 1),
    message(
      1,
      [
        {
          kind: 'toolUse',
          id: 'call_1',
          name: 'AskUserQuestion',
          input: { question: '第一问', options: ['甲', '乙'] },
        },
        {
          kind: 'toolUse',
          id: 'call_2',
          name: 'AskUserQuestion',
          input: { question: '第二问', options: ['丙', '丁'] },
        },
      ],
      null,
      1,
    ),
  ];
}

/** 活卡选项按钮（`ask-answer-option`）文本集合。 */
function liveAskLabels(): string[] {
  return screen.queryAllByTestId('ask-answer-option').map((node) => node.textContent ?? '');
}

/** 一轮对话含两条 ask 工具调用：`call_1` 无结果态（`input-available`）、
 * `call_2` 已配对结果（`output-available`）。 */
function pairedAskEvents(): AgentEvent[] {
  return [
    userMessage(0, '请裁决', 1),
    message(
      1,
      [
        {
          kind: 'toolUse',
          id: 'call_1',
          name: 'AskUserQuestion',
          input: { question: '第一问（未配对）', options: ['甲', '乙'] },
        },
        {
          kind: 'toolUse',
          id: 'call_2',
          name: 'AskUserQuestion',
          input: { question: '第二问（已回灌）', options: ['丙', '丁'] },
        },
        { kind: 'toolResult', id: 'call_2', content: '丁', isError: false },
      ],
      null,
      1,
    ),
  ];
}

describe('AgentMessages：ask 三态判活矩阵', () => {
  it('未提供 pendingToolCallId（undefined）→ 按「无结果态」判活：两条均活卡', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer }}
      />,
    );
    expect(liveAskLabels()).toEqual(['甲', '乙', '丙', '丁']);
  });

  it('未提供 pendingToolCallId（undefined）× 两 state：未配对即活、已配对（output-available）即静', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(pairedAskEvents())}
        loading={false}
        running={false}
        askAnswer={{ onAnswer }}
      />,
    );
    const cards = screen.getAllByTestId('ask-question-card');
    expect(cards).toHaveLength(2);
    const [unpaired, paired] = cards as [HTMLElement, HTMLElement];

    // call_1 仍为无结果态（input-available）→ 按「未配对即活」判活
    expect(
      within(unpaired)
        .queryAllByTestId('ask-answer-option')
        .map((node) => node.textContent),
    ).toEqual(['甲', '乙']);
    expect(within(unpaired).queryAllByTestId('ask-answer-input')).toHaveLength(1);

    // call_2 结果已回灌（output-available）→ 静态只读卡（配对即静）
    expect(within(paired).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(within(paired).queryAllByTestId('ask-answer-input')).toHaveLength(0);
    expect(within(paired).queryAllByTestId('ask-answer-submit')).toHaveLength(0);
    expect(within(paired).queryAllByTestId('ask-option')).toHaveLength(2);
  });

  it('pendingToolCallId = null → 全静态（历史重放零应答入口）', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running={false}
        askAnswer={{ onAnswer, pendingToolCallId: null }}
      />,
    );
    expect(liveAskLabels()).toEqual([]);
    // 静态形态：只读行 + 零输入框
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(4);
    expect(screen.queryAllByTestId('ask-answer-input')).toHaveLength(0);
  });

  it('pendingToolCallId = "call_2" → 仅 call_2 活（三态矩阵的精确对账）', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_2' }}
      />,
    );
    expect(liveAskLabels()).toEqual(['丙', '丁']);
    // call_1 仍为静态只读行（两条只读行在 call_1 卡内）
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(2);
  });

  it('pendingToolCallId 不匹配任何 id → 零活卡（不误呈）', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_404' }}
      />,
    );
    expect(liveAskLabels()).toEqual([]);
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(4);
  });

  it('点活卡选项 → onAnswer 收到选项标签（动作面只达 ask 卡）', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_1' }}
      />,
    );
    fireEvent.click(screen.getAllByTestId('ask-answer-option')[1] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledWith('乙');
  });

  it('pendingToolCallId = ""（空串）→ 不匹配任何部件，全静态且零 panic', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: '' }}
      />,
    );
    // 三态矩阵的退化取值：空串是「已给出但无对账目标」，不是「未给出」
    expect(liveAskLabels()).toEqual([]);
    expect(screen.queryAllByTestId('ask-answer-input')).toHaveLength(0);
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(4);
  });

  it('askAnswer 缺席 → 全静态（既有形态零漂移）', () => {
    render(<AgentMessages messages={fold(twoAskEvents())} loading={false} running={false} />);
    expect(liveAskLabels()).toEqual([]);
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(4);
  });

  it('其它三张内建卡不吃动作面：同轮 Bash 卡零交互面（memo 不被击穿）', () => {
    const onAnswer = vi.fn();
    const events: AgentEvent[] = [
      userMessage(0, '跑一下', 1),
      message(
        1,
        [
          {
            kind: 'toolUse',
            id: 'call_bash',
            name: 'Bash',
            input: { command: 'ls' },
          },
          {
            kind: 'toolUse',
            id: 'call_ask',
            name: 'AskUserQuestion',
            input: { question: '问', options: ['答'] },
          },
        ],
        null,
        1,
      ),
    ];
    render(
      <AgentMessages
        messages={fold(events)}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_ask' }}
      />,
    );
    // 只有 ask 卡有活卡选项；Bash 卡无 ask-answer-option
    expect(liveAskLabels()).toEqual(['答']);
    expect(screen.queryAllByTestId('ask-answer-input')).toHaveLength(1);
  });

  it('仅 ask 部件拿到动作面（AC-11）：同轮三部件中 read / bash 卡零动作面、零按钮差异', () => {
    const onAnswer = vi.fn();
    const events: AgentEvent[] = [
      userMessage(0, '三件同轮', 1),
      message(
        1,
        [
          {
            kind: 'toolUse',
            id: 'call_read',
            name: 'Read',
            input: { path: 'a.md' },
          },
          {
            kind: 'toolUse',
            id: 'call_bash3',
            name: 'Bash',
            input: { command: 'ls' },
          },
          {
            kind: 'toolUse',
            id: 'call_ask3',
            name: 'AskUserQuestion',
            input: { question: '问', options: ['答'] },
          },
        ],
        null,
        1,
      ),
    ];
    cardSpy.refs.length = 0;
    render(
      <AgentMessages
        messages={fold(events)}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_ask3' }}
      />,
    );
    // ask 卡：活卡（选项按钮 + 输入框各一件）
    expect(liveAskLabels()).toEqual(['答']);
    expect(screen.queryAllByTestId('ask-answer-input')).toHaveLength(1);
    // 探针侧证：只有 ask 卡这一张拿到 askAnswer 动作面
    const received = cardSpy.refs.filter((ref) => ref !== null);
    expect(received, '仅 ask 部件收到动作面').toHaveLength(1);
    // read / bash 卡与既有形态逐字相同（零动作面 ⇒ 零新增按钮）：同轮三张卡
    // 里只有 ask 那张带活卡面，其余两张零 ask-answer-* 节点
    const toolIds = screen
      .getAllByTestId('block-tool-use')
      .map((node) => node.getAttribute('data-tool-id'));
    expect(toolIds).toEqual(['call_read', 'call_bash3']);
    for (const node of screen.getAllByTestId('block-tool-use')) {
      expect(within(node).queryAllByTestId('ask-answer-option')).toHaveLength(0);
      expect(within(node).queryAllByTestId('ask-answer-input')).toHaveLength(0);
    }
    // ask 卡自身（`ask-question-card` 卡根）承载唯一活卡面
    const askCard = screen.getByTestId('ask-question-card');
    expect(within(askCard).getAllByTestId('ask-answer-option')).toHaveLength(1);
  });

  it('onAnswer 透传唯一性：多个活卡并存时点任一卡 → onAnswer 恰一次且参数为该卡选项', () => {
    const onAnswer = vi.fn();
    render(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer }}
      />,
    );
    const buttons = screen.getAllByTestId('ask-answer-option');
    expect(buttons.map((node) => node.textContent)).toEqual(['甲', '乙', '丙', '丁']);

    // 点第二张卡（call_2）的选项 → 只投该卡的选项文本，不串卡
    fireEvent.click(buttons[2] as HTMLElement);
    expect(onAnswer.mock.calls).toEqual([['丙']]);
  });

  it('引用稳定（DD12）：每渲染新造 askAnswer 对象但两字段同值 → 下游卡片收到同一引用', () => {
    const onAnswer = vi.fn();
    const messages = fold(twoAskEvents());
    cardSpy.refs.length = 0;
    const { rerender } = render(
      <AgentMessages
        messages={messages}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_1' }}
      />,
    );
    expect(liveAskLabels()).toEqual(['甲', '乙']);
    const firstRefs = [...cardSpy.refs];
    const liveRefs = firstRefs.filter((ref) => ref !== null);
    expect(liveRefs, '活卡张数').toHaveLength(1);

    // 父层重渲染：`askAnswer` 对象引用每次都新造，两字段同值 → `useMemo` 合成同引用
    // ⇒ 卡组件（memo）一帧都不重建（渲染计数判据：重渲染期间零新增调用）
    cardSpy.refs.length = 0;
    rerender(
      <AgentMessages
        messages={messages}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_1' }}
      />,
    );
    expect(cardSpy.refs, '同值重渲染 → 卡组件零重建（引用稳定未击穿 memo）').toHaveLength(0);
    // 合成对象字段面逐字：卡内只认 `onAnswer`（判活在上层完成，卡不下沉 pendingToolCallId）
    expect(liveRefs[0]).toEqual({ onAnswer });
    expect(liveAskLabels()).toEqual(['甲', '乙']);
  });

  it('引用稳定（DD12）：仅父层状态变化（messages 换引用、askAnswer 同值）→ 活卡判活不变', () => {
    const onAnswer = vi.fn();
    const first = fold(twoAskEvents());
    cardSpy.refs.length = 0;
    const { rerender } = render(
      <AgentMessages messages={first} loading={false} running askAnswer={{ onAnswer }} />,
    );
    expect(liveAskLabels()).toEqual(['甲', '乙', '丙', '丁']);

    // messages 换引用（等价内容）——askAnswer 两字段同值，合成引用保持
    cardSpy.refs.length = 0;
    rerender(
      <AgentMessages
        messages={fold(twoAskEvents())}
        loading={false}
        running
        askAnswer={{ onAnswer }}
      />,
    );
    expect(liveAskLabels()).toEqual(['甲', '乙', '丙', '丁']);
  });

  it('引用稳定（DD12）：onAnswer 引用变 → 下游卡片同拍换实现（不冻结旧闭包）', () => {
    const first = vi.fn();
    const second = vi.fn();
    const messages = fold(twoAskEvents());
    cardSpy.refs.length = 0;
    const { rerender } = render(
      <AgentMessages messages={messages} loading={false} running askAnswer={{ onAnswer: first }} />,
    );
    const initial = cardSpy.refs.find((ref) => ref !== null) as NonNullable<AskAnswerActions>;

    cardSpy.refs.length = 0;
    rerender(
      <AgentMessages
        messages={messages}
        loading={false}
        running
        askAnswer={{ onAnswer: second }}
      />,
    );
    const updated = cardSpy.refs.find((ref) => ref !== null) as NonNullable<AskAnswerActions>;

    expect(updated).not.toBe(initial);
    expect(updated.onAnswer).toBe(second);
    fireEvent.click(screen.getAllByTestId('ask-answer-option')[0] as HTMLElement);
    expect(second).toHaveBeenCalledWith('甲');
    expect(first).not.toHaveBeenCalled();
  });

  it('pendingToolCallId 从字符串变 null → 活卡即时收起（不接受控残留）', () => {
    const onAnswer = vi.fn();
    const messages = fold(twoAskEvents());
    const { rerender } = render(
      <AgentMessages
        messages={messages}
        loading={false}
        running
        askAnswer={{ onAnswer, pendingToolCallId: 'call_1' }}
      />,
    );
    expect(liveAskLabels()).toEqual(['甲', '乙']);

    rerender(
      <AgentMessages
        messages={messages}
        loading={false}
        running={false}
        askAnswer={{ onAnswer, pendingToolCallId: null }}
      />,
    );
    expect(liveAskLabels()).toEqual([]);
  });
});
