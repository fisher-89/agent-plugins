import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { UseAgentSessionState } from '@/hooks/use-agent-session';
import type { SessionSummary, TurnSummary } from '@/types/dto';

import { SessionPanel, type SessionPanelState } from './session-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：仅 mock `@tauri-apps/api/core`（invoke + 可编程
// Channel 最小垫片）；`SessionPanelState` 以字面量直构（不经 hook——骨架不自取数
// 即被测契约），回调为 `vi.fn()` 观测。内部模块（AgentSessionView / AgentInput /
// 历史清单局部组件）全真实组合：骨架的呈面语义就是判据。
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

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  ChannelMock.instances.length = 0;
});

describe('SessionPanel：testid 前缀参数化', () => {
  it('两套前缀各自生成全部 13 个查询挂钩且互不串味', () => {
    for (const prefix of PREFIXES) {
      // 组合态：展开 + 清单展开 + 两条历史 + note 非 null
      const { unmount } = render(
        <SessionPanel
          state={panelState({
            sessions: [summary('ses-1'), summary('ses-2')],
            total: 5,
            listOpen: true,
            note: '模型改动不会自动刷新，改完点「刷新」',
          })}
          title="架构 Agent"
          testIdPrefix={prefix}
          composer={() => <div />}
        />,
      );
      for (const name of ['panel', 'actions', 'new', 'history', 'note', 'session']) {
        expect(screen.getByTestId(`${prefix}-${name}`), `${prefix}-${name} 应在场`).toBeTruthy();
      }
      expect(screen.getByTestId(`${prefix}-history-list`)).toBeTruthy();
      expect(screen.getAllByTestId(`${prefix}-history-item`)).toHaveLength(2);
      expect(
        screen.getByTestId(`${prefix}-history-more`),
        'sessions < total → 加载更多',
      ).toBeTruthy();
      unmount();

      // 空清单态：空占位在场（history-empty）
      const empty = render(
        <SessionPanel
          state={panelState({ listOpen: true })}
          title="架构 Agent"
          testIdPrefix={prefix}
          composer={() => <div />}
        />,
      );
      expect(screen.getByTestId(`${prefix}-history-empty`)).toBeTruthy();
      empty.unmount();

      // 错误态：history-error
      const failed = render(
        <SessionPanel
          state={panelState({ listOpen: true, listError: '库不可读' })}
          title="架构 Agent"
          testIdPrefix={prefix}
          composer={() => <div />}
        />,
      );
      expect(screen.getByTestId(`${prefix}-history-error`)).toBeTruthy();
      failed.unmount();

      // 折叠态：rail + expand
      const collapsed = render(
        <SessionPanel
          state={panelState()}
          title="架构 Agent"
          testIdPrefix={prefix}
          composer={() => <div />}
          collapsed
          onToggleCollapsed={vi.fn()}
        />,
      );
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
    const { rerender } = render(
      <SessionPanel
        state={panelState({ newSession, toggleList })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );

    expect(screen.getByTestId('archi-chat-actions').textContent).toContain('架构 Agent');
    fireEvent.click(screen.getByTestId('archi-chat-new'));
    expect(newSession).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId('archi-chat-history'));
    expect(toggleList).toHaveBeenCalledTimes(1);

    expect(screen.getByTestId('archi-chat-history').getAttribute('aria-expanded')).toBe('false');
    rerender(
      <SessionPanel
        state={panelState({ newSession, toggleList, listOpen: true })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.getByTestId('archi-chat-history').getAttribute('aria-expanded')).toBe('true');
  });

  it('canStart 两态：「新会话」禁用时点击零回调，可用时可点', () => {
    const newSession = vi.fn();
    const { rerender } = render(
      <SessionPanel
        state={panelState({ newSession, canStart: false })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    const disabledButton = screen.getByTestId<HTMLButtonElement>('archi-chat-new');
    expect(disabledButton.disabled).toBe(true);
    fireEvent.click(disabledButton);
    expect(newSession, '禁用态点击零回调（骨架只消费该位）').not.toHaveBeenCalled();

    rerender(
      <SessionPanel
        state={panelState({ newSession, canStart: true })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    fireEvent.click(screen.getByTestId('archi-chat-new'));
    expect(newSession).toHaveBeenCalledTimes(1);
  });
});

describe('SessionPanel：状态行只认 note', () => {
  it('note = null 时状态行不在场（不占位、不注入空串）', () => {
    render(
      <SessionPanel
        state={panelState({ note: null })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.queryByTestId('archi-chat-note')).toBeNull();
  });

  it('note 非 null 时逐字呈现（文案生成规则不在骨架内）', () => {
    const note = '模型改动不会自动刷新，改完点「刷新」';
    render(
      <SessionPanel
        state={panelState({ note })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.getByTestId('archi-chat-note').textContent).toBe(note);
  });
});

describe('SessionPanel：历史清单', () => {
  it('展开态两条历史：data-session-id / title 为完整 id，行内为 id 前缀与轮数，点击回调该项 id', () => {
    const selectSession = vi.fn();
    const first = summary('ses-archi-0001', 3);
    const second = summary('ses-archi-0002', 1);
    render(
      <SessionPanel
        state={panelState({
          sessions: [first, second],
          total: 2,
          listOpen: true,
          selectSession,
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );

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
    const { rerender } = render(
      <SessionPanel
        state={panelState({ listOpen: true, listLoading: true })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.getByTestId('archi-chat-history-list').textContent).toContain('加载中…');
    expect(screen.queryByTestId('archi-chat-history-empty')).toBeNull();
    expect(screen.queryByTestId('archi-chat-history-error')).toBeNull();

    rerender(
      <SessionPanel
        state={panelState({ listOpen: true, listError: '库不可读' })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.getByTestId('archi-chat-history-error').textContent).toBe(
      '加载历史会话失败：库不可读',
    );
    expect(screen.queryByTestId('archi-chat-history-empty')).toBeNull();
    expect(screen.getByTestId('archi-chat-history-list').textContent).not.toContain('加载中…');

    rerender(
      <SessionPanel
        state={panelState({ listOpen: true, listLoading: false, listError: null })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.getByTestId('archi-chat-history-empty').textContent).toBe('（暂无历史会话）');
    expect(screen.queryByTestId('archi-chat-history-error')).toBeNull();
  });

  it('分页四形态：items < total 呈「加载更多」可点；加载中禁用；等量 / 超量不呈', () => {
    const loadMore = vi.fn();
    const { rerender } = render(
      <SessionPanel
        state={panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 5,
          listOpen: true,
          loadMore,
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    fireEvent.click(screen.getByTestId('archi-chat-history-more'));
    expect(loadMore).toHaveBeenCalledTimes(1);

    rerender(
      <SessionPanel
        state={panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 5,
          listOpen: true,
          listLoading: true,
          loadMore,
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    const busy = screen.getByTestId<HTMLButtonElement>('archi-chat-history-more');
    expect(busy.disabled, '加载中禁用').toBe(true);
    fireEvent.click(busy);
    expect(loadMore, '禁用态点击零回调').toHaveBeenCalledTimes(1);

    rerender(
      <SessionPanel
        state={panelState({
          sessions: [summary('ses-1'), summary('ses-2')],
          total: 2,
          listOpen: true,
          loadMore,
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.queryByTestId('archi-chat-history-more'), '等量不呈').toBeNull();

    rerender(
      <SessionPanel
        state={panelState({
          sessions: [summary('ses-1'), summary('ses-2'), summary('ses-3')],
          total: 2,
          listOpen: true,
          loadMore,
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
      />,
    );
    expect(screen.queryByTestId('archi-chat-history-more'), '超量（判据 < 严格）不呈').toBeNull();
  });
});

describe('SessionPanel：折叠窄条', () => {
  it('collapsed + onToggleCollapsed 时只渲窄条，展开体全部缺席，点展开回调一次', () => {
    const onToggleCollapsed = vi.fn();
    render(
      <SessionPanel
        state={panelState({ note: '状态行文案' })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
        collapsed
        onToggleCollapsed={onToggleCollapsed}
      />,
    );

    expect(screen.getByTestId('archi-chat-rail').textContent).toContain('架构 Agent');
    const expand = screen.getByTestId('archi-chat-expand');
    expect(expand.getAttribute('title')).toBe('展开架构 Agent');
    for (const absent of ['actions', 'note', 'history-list', 'session']) {
      expect(screen.queryByTestId(`archi-chat-${absent}`), `${absent} 应缺席`).toBeNull();
    }
    fireEvent.click(expand);
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it('折叠但未传回调 / 未折叠但传了回调 → 均渲展开体（无死锁面板）', () => {
    const withoutCallback = render(
      <SessionPanel
        state={panelState()}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
        collapsed
      />,
    );
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
    expect(screen.queryByTestId('archi-chat-expand')).toBeNull();
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    withoutCallback.unmount();

    render(
      <SessionPanel
        state={panelState()}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div />}
        collapsed={false}
        onToggleCollapsed={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
  });
});

describe('SessionPanel：会话体与 composer 插槽', () => {
  it('会话体注入形态在场，composer 收到与会话面同名的四字段 actions', () => {
    const sendMessage = vi.fn();
    const stop = vi.fn();
    const session = sessionState({ running: true, loading: false, sendMessage, stop });
    const seen: unknown[] = [];
    render(
      <SessionPanel
        state={panelState({ session })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={(actions) => {
          seen.push(actions);
          return <div data-testid="archi-chat-composer-probe" />;
        }}
      />,
    );

    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer-probe')).toBeTruthy();
    // `composer` 是 `AgentSessionView` 的 render prop：会话基建自身重评估可多次渲染，
    // 故不作调用次数断言——判据是每次渲染拿到的四字段同源（骨架不另建会话流）。
    expect(seen.length, 'composer 至少被渲染一次').toBeGreaterThan(0);
    for (const [index, actions] of seen.entries()) {
      expect(actions, `第 ${index + 1} 次 composer 渲染的四字段同源`).toEqual({
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
    render(
      <SessionPanel
        state={panelState({
          session,
          sessions: Array.from({ length: 25 }, (_, index) => summary(`ses-${index}`)),
          total: 25,
          canStart: false,
          note: '流程运行中',
        })}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={(actions) => {
          seen.push({ running: actions.running, loading: actions.loading });
          return <div data-testid="archi-chat-composer-probe" />;
        }}
      />,
    );

    expect(seen[0]).toEqual({ running: true, loading: true });
    expect(screen.getByTestId<HTMLButtonElement>('archi-chat-new').disabled).toBe(true);
    expect(screen.getByTestId('archi-chat-note').textContent).toBe('流程运行中');
    expect(screen.queryByTestId('archi-chat-disabled-reason'), '骨架不产出判据文案').toBeNull();
  });
});

describe('SessionPanel：空集合形态', () => {
  it('空清单 / total 0 / 未展开 / note null / 无会话 → 展开体完整不崩', () => {
    render(
      <SessionPanel
        state={panelState()}
        title="架构 Agent"
        testIdPrefix="archi-chat"
        composer={() => <div data-testid="archi-chat-composer-probe" />}
      />,
    );

    expect(screen.getByTestId('archi-chat-panel')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer-probe')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-note')).toBeNull();
    expect(screen.queryByTestId('archi-chat-history-list')).toBeNull();
    expect(invokeMock, '骨架不自取数（零 invoke）').not.toHaveBeenCalled();
  });
});
