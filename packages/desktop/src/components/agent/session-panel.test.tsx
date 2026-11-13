import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ResizablePanelGroup } from '@/components/ui/resizable';
import type { UseAgentSessionState } from '@/hooks/use-agent-session';
import type { SessionSummary, TurnSummary } from '@/types/dto';

import type { AskAnswerActions } from './agent-messages';
import type * as AgentSessionViewModule from './agent-session-view';
import { SessionPanel, type SessionPanelState } from './session-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：仅 mock `@tauri-apps/api/core`（invoke + 可编程
// Channel 最小垫片）；`SessionPanelState` 以字面量直构（不经 hook——骨架不自取数
// 即被测契约），回调为 `vi.fn()` 观测。内部模块（AgentInput / 历史清单局部组件）
// 全真实组合：骨架的呈面语义就是判据；`AgentSessionView` 保留真实实现，仅在其
// 边界挂 props 探针（见下）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// 探针装置（保留真实实现，仅记录 props）：在 `AgentSessionView` 边界把每次渲染
// 实收的 props push 进 `propsLog` 后原样渲染真实组件——「合成的动作面是否等价于
// `state.session.answer`」只能由引用相等判定（活卡可点只证调用，反推不出引用）。
// `importOriginal` 保留全部真实实现（活卡 / 判活 / 气泡照旧），探针零行为。
// ---------------------------------------------------------------------------

const { propsLog } = vi.hoisted(() => ({ propsLog: [] as unknown[] }));

vi.mock('./agent-session-view', async (importOriginal) => {
  const actual = await importOriginal<typeof AgentSessionViewModule>();
  function ProbedAgentSessionView(props: Parameters<typeof actual.AgentSessionView>[0]) {
    propsLog.push(props);
    return <actual.AgentSessionView {...props} />;
  }
  return { ...actual, AgentSessionView: ProbedAgentSessionView };
});

/** 最近一次渲染时下游收到的 `askAnswer`（缺席即 `undefined`；探针零记录即抛错）。 */
function recordedAskAnswer(): AskAnswerActions | undefined {
  const props = propsLog.at(-1);
  if (props === undefined) throw new Error('探针未捕获 AgentSessionView 渲染');
  return (props as { askAnswer?: AskAnswerActions }).askAnswer;
}

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式；时钟不入 mock——updatedAt 由 fixture 显式给定）
// ---------------------------------------------------------------------------

const TS = 1_727_000_000_000;

function turn(turnId: number): TurnSummary {
  return {
    turnId,
    sessionId: 'ses-1',
    status: 'completed',
    startedAt: TS + turnId,
    finishedAt: TS + turnId + 900,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    error: null,
  };
}

function summary(id: string, turnCount = 1): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'archi', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: {
      turnCount,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
    },
    turns: Array.from({ length: turnCount }, (_, index) => turn(index + 1)),
  };
}

/** 会话面（注入形态）：空态缺省 = 无会话 / 空消息 / 空事件 / 空链。 */
function sessionState(overrides: Partial<UseAgentSessionState> = {}): UseAgentSessionState {
  return {
    messages: [],
    events: [],
    session: null,
    chain: [],
    loading: false,
    running: false,
    error: null,
    sendMessage: vi.fn(),
    stop: vi.fn(),
    reset: vi.fn(),
    answer: vi.fn(),
    steer: vi.fn(),
    steering: false,
    ...overrides,
  };
}

/** 骨架状态底座（展开态 / 空清单 / 无状态行 / 可起会话）。 */
function panelState(overrides: Partial<SessionPanelState> = {}): SessionPanelState {
  return {
    session: sessionState(),
    sessions: [],
    total: 0,
    listOpen: false,
    listLoading: false,
    listError: null,
    toggleList: vi.fn(),
    loadMore: vi.fn(),
    newSession: vi.fn(),
    selectSession: vi.fn(),
    canStart: true,
    note: null,
    ...overrides,
  };
}

const PREFIXES = ['change-chat', 'archi-chat'] as const;

interface MountOptions {
  state: SessionPanelState;
  title?: string;
  testIdPrefix?: string;
  composer?: (actions: AgentSessionViewModule.AgentSessionActions) => React.ReactNode;
}

/**
 * 骨架自身即 `ResizablePanel`（分栏子面板），须挂在 `ResizablePanelGroup` 内；
 * 挂载默认收起（只呈窄条），故测试以 `mountExpanded` 进入展开体。
 */
function panelUi({
  state,
  title = '架构 Agent',
  testIdPrefix = 'archi-chat',
  composer,
}: MountOptions): React.JSX.Element {
  return (
    <ResizablePanelGroup orientation="horizontal">
      <SessionPanel
        state={state}
        title={title}
        testIdPrefix={testIdPrefix}
        composer={composer ?? (() => <div />)}
      />
    </ResizablePanelGroup>
  );
}

function mountPanel(options: MountOptions) {
  return render(panelUi(options));
}

/** 挂载并点窄条展开入口进入展开体（骨架默认收起）。 */
function mountExpanded(options: MountOptions): ReturnType<typeof render> {
  const rendered = mountPanel(options);
  fireEvent.click(screen.getByTestId(`${options.testIdPrefix ?? 'archi-chat'}-expand`));
  return rendered;
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  ChannelMock.instances.length = 0;
  propsLog.length = 0;
});

describe('SessionPanel：testid 前缀参数化', () => {
  it('两套前缀各自生成全部 13 个查询挂钩且互不串味', () => {
    for (const prefix of PREFIXES) {
      // 组合态：展开 + 清单展开 + 两条历史 + note 非 null
      const expanded = mountExpanded({
        state: panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 5,
          listOpen: true,
          note: '模型改动不会自动刷新，改完点「刷新」',
        }),
        testIdPrefix: prefix,
      });
      for (const name of ['actions', 'collapse', 'new', 'history', 'note', 'session']) {
        expect(screen.getByTestId(`${prefix}-${name}`), `${prefix}-${name} 应在场`).toBeTruthy();
      }
      expect(screen.getByTestId(`${prefix}-history-list`)).toBeTruthy();
      expect(screen.getAllByTestId(`${prefix}-history-item`)).toHaveLength(2);
      expect(
        screen.getByTestId(`${prefix}-history-more`),
        'sessions < total → 加载更多',
      ).toBeTruthy();
      expanded.unmount();

      // 空清单态：空占位在场（history-empty）
      const empty = mountExpanded({ state: panelState({ listOpen: true }), testIdPrefix: prefix });
      expect(screen.getByTestId(`${prefix}-history-empty`)).toBeTruthy();
      empty.unmount();

      // 错误态：history-error
      const failed = mountExpanded({
        state: panelState({ listOpen: true, listError: '库不可读' }),
        testIdPrefix: prefix,
      });
      expect(screen.getByTestId(`${prefix}-history-error`)).toBeTruthy();
      failed.unmount();

      // 收起态：rail + expand
      const collapsed = mountPanel({ state: panelState(), testIdPrefix: prefix });
      expect(screen.getByTestId(`${prefix}-rail`)).toBeTruthy();
      expect(screen.getByTestId(`${prefix}-expand`)).toBeTruthy();
      collapsed.unmount();
    }
  });
});

describe('SessionPanel：头区动作', () => {
  it('title 呈现于头区，点新会话 / 历史各调一次回调且 aria-expanded 随 listOpen', () => {
    const newSession = vi.fn();
    const toggleList = vi.fn();
    const { rerender } = mountExpanded({
      state: panelState({ newSession, toggleList }),
      testIdPrefix: 'archi-chat',
    });

    expect(screen.getByTestId('archi-chat-actions').textContent).toContain('架构 Agent');
    fireEvent.click(screen.getByTestId('archi-chat-new'));
    expect(newSession).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId('archi-chat-history'));
    expect(toggleList).toHaveBeenCalledTimes(1);

    expect(screen.getByTestId('archi-chat-history').getAttribute('aria-expanded')).toBe('false');
    rerender(panelUi({ state: panelState({ newSession, toggleList, listOpen: true }) }));
    expect(screen.getByTestId('archi-chat-history').getAttribute('aria-expanded')).toBe('true');
  });

  it('canStart 两态：「新会话」禁用时点击零回调，可用时可点', () => {
    const newSession = vi.fn();
    const { rerender } = mountExpanded({
      state: panelState({ newSession, canStart: false }),
      testIdPrefix: 'archi-chat',
    });
    const disabledButton = screen.getByTestId<HTMLButtonElement>('archi-chat-new');
    expect(disabledButton.disabled).toBe(true);
    fireEvent.click(disabledButton);
    expect(newSession, '禁用态点击零回调（骨架只消费该位）').not.toHaveBeenCalled();

    rerender(panelUi({ state: panelState({ newSession, canStart: true }) }));
    fireEvent.click(screen.getByTestId('archi-chat-new'));
    expect(newSession).toHaveBeenCalledTimes(1);
  });
});

describe('SessionPanel：状态行只认 note', () => {
  it('note = null 时状态行不在场（不占位、不注入空串）', () => {
    mountExpanded({ state: panelState({ note: null }), testIdPrefix: 'archi-chat' });
    expect(screen.queryByTestId('archi-chat-note')).toBeNull();
  });

  it('note 非 null 时逐字呈现（文案生成规则不在骨架内）', () => {
    const note = '模型改动不会自动刷新，改完点「刷新」';
    mountExpanded({ state: panelState({ note }), testIdPrefix: 'archi-chat' });
    expect(screen.getByTestId('archi-chat-note').textContent).toBe(note);
  });
});

describe('SessionPanel：历史清单', () => {
  it('展开态两条历史：data-session-id / title 为完整 id，行内为 id 前缀与轮数，点击回调该项 id', () => {
    const selectSession = vi.fn();
    const first = summary('ses-archi-0001', 3);
    const second = summary('ses-archi-0002', 1);
    mountExpanded({
      state: panelState({
        sessions: [first, second],
        total: 2,
        listOpen: true,
        selectSession,
      }),
      testIdPrefix: 'archi-chat',
    });

    const items = screen.getAllByTestId('archi-chat-history-item');
    expect(items).toHaveLength(2);
    expect(items[0].getAttribute('data-session-id')).toBe('ses-archi-0001');
    expect(items[0].getAttribute('title')).toBe('ses-archi-0001');
    expect(items[0].textContent).toContain('ses-arch');
    expect(items[0].textContent).not.toContain('ses-archi-0001');
    expect(items[0].textContent).toContain('3 轮');
    expect(items[1].textContent).toContain('1 轮');

    fireEvent.click(items[0]);
    expect(selectSession).toHaveBeenCalledTimes(1);
    expect(selectSession).toHaveBeenCalledWith('ses-archi-0001');
  });

  it('加载中 / 错误 / 空态三互斥：同一拍只呈其一', () => {
    const { rerender } = mountExpanded({
      state: panelState({ listOpen: true, listLoading: true }),
      testIdPrefix: 'archi-chat',
    });
    expect(screen.getByTestId('archi-chat-history-list').textContent).toContain('加载中…');
    expect(screen.queryByTestId('archi-chat-history-empty')).toBeNull();
    expect(screen.queryByTestId('archi-chat-history-error')).toBeNull();

    rerender(panelUi({ state: panelState({ listOpen: true, listError: '库不可读' }) }));
    expect(screen.getByTestId('archi-chat-history-error').textContent).toBe(
      '加载历史会话失败：库不可读',
    );
    expect(screen.queryByTestId('archi-chat-history-empty')).toBeNull();
    expect(screen.getByTestId('archi-chat-history-list').textContent).not.toContain('加载中…');

    rerender(
      panelUi({ state: panelState({ listOpen: true, listLoading: false, listError: null }) }),
    );
    expect(screen.getByTestId('archi-chat-history-empty').textContent).toBe('（暂无历史会话）');
    expect(screen.queryByTestId('archi-chat-history-error')).toBeNull();
  });

  it('分页四形态：items < total 呈「加载更多」可点；加载中禁用；等量 / 超量不呈', () => {
    const loadMore = vi.fn();
    const { rerender } = mountExpanded({
      state: panelState({
        sessions: [summary('ses-1'), summary('ses-2')],
        total: 5,
        listOpen: true,
        loadMore,
      }),
      testIdPrefix: 'archi-chat',
    });
    fireEvent.click(screen.getByTestId('archi-chat-history-more'));
    expect(loadMore).toHaveBeenCalledTimes(1);

    rerender(
      panelUi({
        state: panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 5,
          listOpen: true,
          listLoading: true,
          loadMore,
        }),
      }),
    );
    const busy = screen.getByTestId<HTMLButtonElement>('archi-chat-history-more');
    expect(busy.disabled, '加载中禁用').toBe(true);
    fireEvent.click(busy);
    expect(loadMore, '禁用态点击零回调').toHaveBeenCalledTimes(1);

    rerender(
      panelUi({
        state: panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 2,
          listOpen: true,
          loadMore,
        }),
      }),
    );
    expect(screen.queryByTestId('archi-chat-history-more'), '等量不呈').toBeNull();

    rerender(
      panelUi({
        state: panelState({
          sessions: [summary('ses-1'), summary('ses-2'), summary('ses-3')],
          total: 2,
          listOpen: true,
          loadMore,
        }),
      }),
    );
    expect(screen.queryByTestId('archi-chat-history-more'), '超量（判据 < 严格）不呈').toBeNull();
  });
});

describe('SessionPanel：折叠窄条（骨架自持折叠态）', () => {
  it('挂载默认收起：只渲窄条，展开体全部缺席，点展开入口进入展开体', () => {
    mountPanel({ state: panelState({ note: '状态行文案' }), testIdPrefix: 'archi-chat' });

    expect(screen.getByTestId('archi-chat-rail').textContent).toContain('架构 Agent');
    const expand = screen.getByTestId('archi-chat-expand');
    expect(expand.getAttribute('title')).toBe('展开架构 Agent');
    for (const absent of ['actions', 'note', 'history-list', 'session']) {
      expect(screen.queryByTestId(`archi-chat-${absent}`), `${absent} 应缺席`).toBeNull();
    }

    fireEvent.click(expand);
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-note').textContent).toBe('状态行文案');
  });

  it('头区「收起Agent」→ 回窄条；窄条展开入口 → 再展开（互逆，无死锁面板）', () => {
    mountExpanded({ state: panelState(), testIdPrefix: 'archi-chat' });
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();

    fireEvent.click(screen.getByTestId('archi-chat-collapse'));
    expect(screen.getByTestId('archi-chat-rail')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-expand')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-actions')).toBeNull();

    fireEvent.click(screen.getByTestId('archi-chat-expand'));
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
    expect(invokeMock, '折叠切换零取数').not.toHaveBeenCalled();
  });
});

describe('SessionPanel：会话体与 composer 插槽', () => {
  it('会话体注入形态在场，composer 收到与会话面同名的四字段 actions', () => {
    const sendMessage = vi.fn();
    const stop = vi.fn();
    const session = sessionState({ running: true, loading: false, sendMessage, stop });
    const seen: unknown[] = [];
    mountExpanded({
      state: panelState({ session }),
      testIdPrefix: 'archi-chat',
      composer: (actions) => {
        seen.push(actions);
        return <div data-testid="archi-chat-composer-probe" />;
      },
    });

    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer-probe')).toBeTruthy();
    // `composer` 是 `AgentSessionView` 的 render prop：会话基建自身重评估可多次渲染，
    // 故不作调用次数断言——判据是每次渲染拿到的四字段同源（骨架不另建会话流）。
    expect(seen.length, 'composer 至少被渲染一次').toBeGreaterThan(0);
    for (const [index, actions] of seen.entries()) {
      expect(actions, `第 ${index + 1} 次 composer 渲染的四字段同源`).toMatchObject({
        running: true,
        loading: false,
        send: sendMessage,
        stop,
      });
    }
  });

  it('骨架零业务判据：看似禁发的外部状态不被改写（原值透传）', () => {
    const session = sessionState({
      running: true,
      loading: true,
      error: '上一轮失败',
      session: summary('ses-live'),
    });
    const seen: { running: boolean; loading: boolean }[] = [];
    mountExpanded({
      state: panelState({
        session,
        sessions: Array.from({ length: 25 }, (_, index) => summary(`ses-${index}`)),
        total: 25,
        canStart: false,
        note: '流程运行中',
      }),
      testIdPrefix: 'archi-chat',
      composer: (actions) => {
        seen.push({ running: actions.running, loading: actions.loading });
        return <div data-testid="archi-chat-composer-probe" />;
      },
    });

    expect(seen[0]).toEqual({ running: true, loading: true });
    expect(screen.getByTestId<HTMLButtonElement>('archi-chat-new').disabled).toBe(true);
    expect(screen.getByTestId('archi-chat-note').textContent).toBe('流程运行中');
    expect(screen.queryByTestId('archi-chat-disabled-reason'), '骨架不产出判据文案').toBeNull();
  });
});

describe('SessionPanel：空集合形态', () => {
  it('空清单 / total 0 / 未展开 / note null / 无会话 → 展开体完整不崩', () => {
    mountExpanded({
      state: panelState(),
      testIdPrefix: 'archi-chat',
      composer: () => <div data-testid="archi-chat-composer-probe" />,
    });

    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer-probe')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-note')).toBeNull();
    expect(screen.queryByTestId('archi-chat-history-list')).toBeNull();
    expect(invokeMock, '骨架不自取数（零 invoke）').not.toHaveBeenCalled();
  });
});

describe('SessionPanel：ask 动作面合成（骨架零改动即得活卡）', () => {
  /** 一轮含 ask 工具调用消息（活卡判据面：无结果的 `input-available` 部件）。 */
  function askMessages(): UseAgentSessionState['messages'] {
    return [
      {
        id: 'm-ask',
        role: 'assistant',
        parts: [
          {
            type: 'tool-AskUserQuestion',
            toolCallId: 'call_1',
            state: 'input-available',
            input: { question: '选哪个重试策略？', options: ['指数退避', '固定间隔'] },
          },
        ],
      } as unknown as UseAgentSessionState['messages'][number],
    ];
  }

  it('展开体把 state.session.answer 合成 askAnswer 下传：ask 卡提交即触发 answer', () => {
    const answer = vi.fn();
    const session = sessionState({ answer, messages: askMessages() });
    mountExpanded({
      state: panelState({ session }),
      testIdPrefix: 'archi-chat',
    });

    const buttons = screen.getAllByTestId('ask-answer-option');
    expect(buttons.map((node) => node.textContent)).toEqual(['指数退避', '固定间隔']);
    // 探针判据：下游收到的 `onAnswer` 就是该 `answer` 引用
    expect(recordedAskAnswer()?.onAnswer).toBe(answer);
    fireEvent.click(buttons[0] as HTMLElement);
    fireEvent.click(screen.getByTestId('ask-answer-submit'));
    expect(answer.mock.calls).toEqual([['{"Q1":"指数退避"}']]);
  });

  it('两套前缀（change-chat / archi-chat）展开体均合成动作面且互不串味', () => {
    for (const prefix of PREFIXES) {
      const answer = vi.fn();
      const mounted = mountExpanded({
        state: panelState({ session: sessionState({ answer, messages: askMessages() }) }),
        testIdPrefix: prefix,
      });
      const buttons = screen.getAllByTestId('ask-answer-option');
      fireEvent.click(buttons[0] as HTMLElement);
      fireEvent.click(screen.getByTestId('ask-answer-submit'));
      expect(answer).toHaveBeenCalledTimes(1);
      expect(answer).toHaveBeenCalledWith('{"Q1":"指数退避"}');
      mounted.unmount();
    }
  });

  it('引用等价（DD12）：合成 askAnswer.onAnswer 与 state.session.answer 为同一引用', () => {
    const answer = vi.fn();
    const state = panelState({ session: sessionState({ answer, messages: askMessages() }) });
    const { rerender } = mountExpanded({ state, testIdPrefix: 'archi-chat' });

    // 探针判据：合成对象的外层每次渲染新造（`{ onAnswer: ... }` 字面量），但字段
    // `onAnswer` 与 `state.session.answer` 同引用 ⇒ 下游 `AgentMessages` 的
    // `useMemo([onAnswer, pendingToolCallId])` 合成引用稳定、卡组件一帧都不重建
    // （骨架不另包一层回调、不击穿 memo）
    const first = recordedAskAnswer();
    expect(first?.onAnswer).toBe(answer);

    rerender(panelUi({ state, testIdPrefix: 'archi-chat' }));
    const second = recordedAskAnswer();
    expect(propsLog.length, '重渲染确实到达下游边界').toBeGreaterThan(1);
    expect(second?.onAnswer).toBe(answer);
    expect(second?.onAnswer, '两次渲染字段同引用').toBe(first?.onAnswer);

    // 行为侧证：提交直接以该引用投递（合成对象不额外包装）
    fireEvent.click(screen.getAllByTestId('ask-answer-option')[1] as HTMLElement);
    fireEvent.click(screen.getByTestId('ask-answer-submit'));
    expect(answer.mock.calls).toEqual([['{"Q1":"固定间隔"}']]);
  });

  it('无 ask 部件时会话体照常呈现（动作面在场零副作用）', () => {
    const answer = vi.fn();
    mountExpanded({
      state: panelState({ session: sessionState({ answer }) }),
      testIdPrefix: 'archi-chat',
    });
    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.queryAllByTestId('ask-answer-submit')).toHaveLength(0);
    expect(answer).not.toHaveBeenCalled();
  });

  it('不注入 pendingToolCallId（DD8 三态语义）：合成对象键面只 onAnswer → 按未配对判活', () => {
    const answer = vi.fn();
    mountExpanded({
      state: panelState({ session: sessionState({ answer, messages: askMessages() }) }),
      testIdPrefix: 'archi-chat',
    });

    // 行为侧证：`input-available` 部件（无结果态）在骨架合成下呈活卡
    expect(screen.getAllByTestId('ask-answer-option').length).toBeGreaterThan(0);

    // 探针判据：合成对象键面只有 `onAnswer` 一键（`pendingToolCallId` 缺席 ⇒ 判活落
    // `undefined` 分支 = 未配对即活）；骨架不注入判活 id
    const askAnswer = recordedAskAnswer();
    expect(Object.keys(askAnswer ?? {}), '合成对象只有 onAnswer 一键').toEqual(['onAnswer']);
    expect('pendingToolCallId' in (askAnswer ?? {}), '骨架不注入判活 id').toBe(false);
    expect(askAnswer?.onAnswer).toBe(answer);

    // 下游每拍实收的 props 都无 `pendingToolCallId` 键（合成对象不在透传路上被补字段）
    for (const record of propsLog as { askAnswer?: Record<string, unknown> }[]) {
      expect('pendingToolCallId' in (record.askAnswer ?? {})).toBe(false);
    }
  });
});
