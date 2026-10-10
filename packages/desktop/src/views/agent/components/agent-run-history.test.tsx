// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import type { AgentRunHistoryState } from '../hooks/use-agent-run-history';
import { AgentRunHistory } from './agent-run-history';

// state 对象（AgentRunHistoryState）以 fixture 直传，无进程边界，不需要 Mock。

const TS = 1727000000000;

function turn(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: `ses-${turnId}-1727000000000`,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 999,
    numTurns: 4,
    costUsd: 0.2,
    durationMs: 800,
    error: status === 'failed' ? '进程结束但未产出收敛事件' : null,
  };
}

function session(
  id: string,
  status: TurnSummary['status'],
  updatedAt: number,
  turns: TurnSummary[] = [turn(1, status)],
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: { engine: 'sdk', model: 'glm-high', permissionMode: 'bypassPermissions' },
      provenance: { source: 'debug', sourceRef: null },
      createdAt: TS,
      updatedAt,
    },
    stats: { turnCount: turns.length, totalDurationMs: 800, inputTokens: null, outputTokens: null },
    turns,
  };
}

function event(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 'ses-0-1727000000000',
    tools: [],
    mcpServers: [],
  };
}

/** 密封文本消息事件（对话透镜可见体；runStarted 为辅助载体不进对话呈现）。 */
function chatEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function historyState(overrides: Partial<AgentRunHistoryState> = {}): AgentRunHistoryState {
  return {
    sessions: [],
    events: [],
    selectedSessionId: null,
    loading: false,
    error: null,
    refresh: vi.fn(),
    openSession: vi.fn(),
    ...overrides,
  };
}

function mount(state: AgentRunHistoryState) {
  return render(<AgentRunHistory state={state} source="debug" onSourceChange={() => {}} />);
}

describe('AgentRunHistory：会话列表与触发（AC-5 / D13）', () => {
  it('sessions 渲染终态 / 轮数 / 时间三要素（会话行 = 最新轮终态 + 现算轮数）', () => {
    mount(
      historyState({
        sessions: [
          session('ses-1-1727000000000', 'completed', TS + 1),
          session('ses-2-1727000000000', 'failed', TS + 2),
        ],
      }),
    );

    const rows = screen.getAllByTestId('agent-run-row');
    expect(rows).toHaveLength(2);
    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['已完成', '失败']);
    expect(rows[0]?.textContent).toContain('1 轮');
    // 时间以本地时区字符串呈现（非空即可，不绑定格式）
    expect((rows[0]?.textContent ?? '').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('run-row-error')).toHaveLength(1);
    expect(screen.getAllByTestId('run-row-error')[0]?.textContent).toContain(
      '进程结束但未产出收敛事件',
    );
    // 头行计数随清单长度（非恒 0/1 硬编码）
    expect(screen.getByTestId('agent-run-history').textContent).toContain('历史会话 (2)');
  });

  it('最新轮行 status 为 stopped 的会话行状态标签呈「已停止」，其余三态标签不回归', () => {
    mount(
      historyState({
        sessions: [
          session('ses-1-1727000000000', 'running', TS + 1),
          session('ses-2-1727000000000', 'completed', TS + 2),
          session('ses-3-1727000000000', 'stopped', TS + 3),
          session('ses-4-1727000000000', 'failed', TS + 4),
        ],
      }),
    );

    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['运行中', '已完成', '已停止', '失败']);
    const stoppedRow = screen
      .getAllByTestId('agent-run-row')
      .find((row) => row.getAttribute('data-status') === 'stopped');
    expect(stoppedRow?.getAttribute('data-session-id')).toBe('ses-3-1727000000000');
    // 徽章配色随终态可辨：completed 绿 / failed 红 / running 灰（三元链逐臂）
    const rows = screen.getAllByTestId('agent-run-row');
    const statusBadges = rows.map((row) => within(row).getByTestId('run-status'));
    expect(statusBadges[0]?.className).toContain('text-muted-foreground');
    expect(statusBadges[1]?.className).toContain('text-pass');
    expect(statusBadges[3]?.className).toContain('text-fail');
    expect(statusBadges[2]?.className).toContain('text-muted-foreground');
    expect(statusBadges[1]?.className).not.toContain('text-fail');
  });

  it('终态取自最新轮行（非首行轮）：completed 后接 failed 轮 → 行徽标随末轮呈失败', () => {
    const id = 'ses-8-1727000000000';
    const multi: SessionSummary = {
      ...session(id, 'failed', TS + 8, [turn(1, 'completed'), turn(2, 'failed')]),
      stats: { turnCount: 5, totalDurationMs: 800, inputTokens: null, outputTokens: null },
    };
    mount(historyState({ sessions: [multi] }));

    const row = screen.getByTestId('agent-run-row');
    expect(row.getAttribute('data-status')).toBe('failed');
    expect(within(row).getByTestId('run-status').textContent).toBe('失败');
    expect(within(row).getByTestId('run-status').className).toContain('text-fail');
    // 轮数取 stats.turnCount 现算值（非 turns 数组长度投影）
    expect(row.textContent).toContain('5 轮');
    // 末轮 error 随行呈现
    expect(within(row).getByTestId('run-row-error').textContent).toContain(
      '进程结束但未产出收敛事件',
    );
  });

  it('无轮行空会话：徽标「空会话」灰显、data-status empty、无错误行（空态词汇不缺省）', () => {
    const id = 'ses-9-1727000000000';
    mount(historyState({ sessions: [session(id, 'completed', TS + 9, [])] }));

    const row = screen.getByTestId('agent-run-row');
    expect(row.getAttribute('data-status')).toBe('empty');
    const badge = within(row).getByTestId('run-status');
    expect(badge.textContent).toBe('空会话');
    expect(badge.className).toContain('text-muted-foreground');
    expect(within(row).queryByTestId('run-row-error')).toBeNull();
  });

  it('点击会话行 → openSession(sessionId) 恰调用一次（重放触发半边）', () => {
    const openSession = vi.fn();
    const sessionId = 'ses-7-1727000000000';
    mount(historyState({ sessions: [session(sessionId, 'completed', TS + 7)], openSession }));

    fireEvent.click(screen.getByTestId('agent-run-row'));

    expect(openSession).toHaveBeenCalledTimes(1);
    expect(openSession).toHaveBeenCalledWith(sessionId);
  });

  it('点击刷新 → refresh() 恰调用一次（D13 显式触发）', () => {
    const refresh = vi.fn();
    mount(historyState({ sessions: [session('ses-1-1727000000000', 'completed', TS)], refresh }));

    fireEvent.click(screen.getByTestId('history-refresh'));

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('selectedSessionId 匹配时呈现重放区并还原全史对话（与实时流同组件）', () => {
    const sessionId = 'ses-7-1727000000000';
    mount(
      historyState({
        sessions: [session(sessionId, 'completed', TS + 7)],
        events: [event(0), chatEvent(1, '重放转录正文')],
        selectedSessionId: sessionId,
      }),
    );

    const replay = screen.getByTestId('replay-area');
    expect(within(replay).getByTestId('agent-messages') !== null).toBe(true);
    // 密封转录经同一对话透镜还原（runStarted 等辅助载体不进对话呈现）
    expect(within(replay).getByTestId('block-text').textContent).toContain('重放转录正文');
    // 密封重放无流式尾态：running 恒 false（无 agent-messages-running 标记）
    expect(within(replay).queryByTestId('agent-messages-running')).toBeNull();
  });
});

describe('AgentRunHistory：空态 / loading / error（边界与异常）', () => {
  it('sessions 为空时呈现空态文案不崩', () => {
    mount(historyState());

    expect(screen.getByTestId('history-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('agent-run-row')).toHaveLength(0);
  });

  it('loading=true 呈现加载态', () => {
    const refresh = vi.fn();
    mount(historyState({ loading: true, refresh }));

    expect(screen.getByTestId('history-loading') !== null).toBe(true);
    // 取数进行中刷新入口禁用（disabled 随 loading）
    expect(screen.getByTestId('history-refresh')).toHaveProperty('disabled', true);
    // 无选中会话时不渲染重放区（selectedSessionId null 不越权呈现）
    expect(screen.queryByTestId('replay-area')).toBeNull();
    // 加载中不出空态文案、不出错误条（空态三条件随状态收口）
    expect(screen.queryByTestId('history-empty')).toBeNull();
    expect(screen.queryByTestId('history-error')).toBeNull();
  });

  it('error 非空呈现错误条；列表仍可用（行照常渲染）', () => {
    const refresh = vi.fn();
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        error: 'db: 转录读取失败',
        refresh,
      }),
    );

    expect(screen.getByTestId('history-error').textContent).toContain('db: 转录读取失败');
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    // 错误态下刷新入口仍可触发
    fireEvent.click(screen.getByTestId('history-refresh'));
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('error 非空且清单为空：错误条呈现、空态文案不并存（空态被 error 门禁收口）', () => {
    mount(historyState({ sessions: [], error: 'db: 清单打开失败' }));

    expect(screen.getByTestId('history-error') !== null).toBe(true);
    expect(screen.queryByTestId('history-empty')).toBeNull();
    expect(screen.queryByTestId('history-loading')).toBeNull();
  });

  it('error 为 null：错误条缺席（无错不渲染错误横幅）', () => {
    mount(historyState({ sessions: [session('ses-1-1727000000000', 'completed', TS)] }));

    expect(screen.queryByTestId('history-error')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 来源筛选三态按钮组（desktop-change-session-visibility，AC-3）：头行
// debug / change / 全部三枚按钮（data-testid 挂钩 + data-source + aria-pressed
// 高亮随 props 切换——组件无状态，状态提升断言面）。
// ---------------------------------------------------------------------------

describe('AgentRunHistory：来源筛选三态按钮组（AC-3）', () => {
  it("source='debug' 挂载：history-source-filter 内三枚按钮 data-source 为 debug / change / all，debug 枚 aria-pressed 高亮", () => {
    mount(historyState());

    const filter = screen.getByTestId('history-source-filter');
    const buttons = within(filter).getAllByRole('button');
    expect(buttons.map((button) => button.getAttribute('data-source'))).toEqual([
      'debug',
      'change',
      'all',
    ]);
    expect(buttons.map((button) => button.textContent)).toEqual(['调试', '变更', '全部']);
    expect(buttons.map((button) => button.getAttribute('aria-pressed'))).toEqual([
      'true',
      'false',
      'false',
    ]);
    // 高亮态着色随 aria-pressed：选中项无灰化类，未选中项带灰化类（三元两臂）
    expect(buttons[0]?.className).not.toContain('bg-muted');
    expect(buttons[1]?.className).toContain('bg-muted');
    expect(buttons[2]?.className).toContain('bg-muted');
  });

  it("点击 change 枚 → onSourceChange('change') 恰调用一次（组件无状态——状态提升断言面）", () => {
    const onSourceChange = vi.fn();
    render(
      <AgentRunHistory state={historyState()} source="debug" onSourceChange={onSourceChange} />,
    );

    const buttons = within(screen.getByTestId('history-source-filter')).getAllByRole('button');
    fireEvent.click(buttons[1]);

    expect(onSourceChange).toHaveBeenCalledTimes(1);
    expect(onSourceChange).toHaveBeenCalledWith('change');
  });

  it("source='all' 挂载：all 枚 aria-pressed 高亮（高亮随 props 切换，组件不自持状态）", () => {
    render(<AgentRunHistory state={historyState()} source="all" onSourceChange={() => {}} />);

    const pressed = within(screen.getByTestId('history-source-filter'))
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-pressed'));
    expect(pressed).toEqual(['false', 'false', 'true']);
  });

  it('筛选按钮组与列表 / 刷新 / 重放区并存：挂载头行既含筛选又含刷新（布局零改动回归）', () => {
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
      }),
    );

    expect(screen.getByTestId('history-source-filter') !== null).toBe(true);
    expect(screen.getByTestId('history-refresh') !== null).toBe(true);
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    // 清单非空时不出空态文案（空态仅限零清单）
    expect(screen.queryByTestId('history-empty')).toBeNull();
  });
});
