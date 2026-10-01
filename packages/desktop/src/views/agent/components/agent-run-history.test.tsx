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
  render(<AgentRunHistory state={state} />);
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

  it('selectedSessionId 匹配时呈现重放区并还原全史时间线（与实时流同组件）', () => {
    const sessionId = 'ses-7-1727000000000';
    mount(
      historyState({
        sessions: [session(sessionId, 'completed', TS + 7)],
        events: [event(0)],
        selectedSessionId: sessionId,
      }),
    );

    const replay = screen.getByTestId('replay-area');
    expect(replay.getAttribute('data-selected-run-id')).toBe(sessionId);
    expect(within(replay).getByTestId('agent-timeline') !== null).toBe(true);
    expect(within(replay).getByTestId('event-run-started') !== null).toBe(true);
  });

  it('重放区限高滚动：max-h-96 overflow-y-auto 容器，内嵌 AgentTimeline 无 props 分叉', () => {
    const sessionId = 'ses-7-1727000000000';
    mount(
      historyState({
        sessions: [session(sessionId, 'completed', TS + 7)],
        events: [event(0), event(1)],
        selectedSessionId: sessionId,
      }),
    );

    // 重放区不再撑高页面：限高容器自身滚动
    const replay = screen.getByTestId('replay-area');
    expect(replay.className).toContain('max-h-96');
    expect(replay.className).toContain('overflow-y-auto');
    // 内嵌 AgentTimeline 与流式形态同组件同结构（普通块容器内自然高、无 props 分叉）
    const timeline = within(replay).getByTestId('agent-timeline');
    expect(timeline.className).toContain('flex-1');
    expect(within(timeline).getAllByTestId('event-run-started')).toHaveLength(2);
  });
});

describe('AgentRunHistory：空态 / loading / error（边界与异常）', () => {
  it('sessions 为空时呈现空态文案不崩', () => {
    mount(historyState());

    expect(screen.getByTestId('history-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('agent-run-row')).toHaveLength(0);
  });

  it('loading=true 呈现加载态', () => {
    mount(historyState({ loading: true }));

    expect(screen.getByTestId('history-loading') !== null).toBe(true);
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
});
