// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../../../types/dto';
import type { AgentRunHistoryState } from '../hooks/use-agent-run-history';
import { AgentRunHistory } from './agent-run-history';

// state 对象（AgentRunHistoryState）以 fixture 直传，无进程边界，不需要 Mock。

function record(id: number, status: AgentRunRecord['status'], prompt: string): AgentRunRecord {
  return {
    id,
    prompt,
    cwd: 'C:\\demo\\beta',
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000 + id,
    finishedAt: null,
    numTurns: 4,
    costUsd: 0.2,
    durationMs: 800,
    sessionId: `s-${id}`,
    error: status === 'failed' ? '进程结束但未产出 result 事件' : null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
  };
}

function event(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: [],
    mcpServers: [],
  };
}

function historyState(overrides: Partial<AgentRunHistoryState> = {}): AgentRunHistoryState {
  return {
    runs: [],
    events: [],
    selectedRunId: null,
    loading: false,
    error: null,
    refresh: vi.fn(),
    openRun: vi.fn(),
    ...overrides,
  };
}

function mount(state: AgentRunHistoryState) {
  render(<AgentRunHistory state={state} />);
}

describe('AgentRunHistory：run 列表与触发（AC-5 / D13）', () => {
  it('runs 渲染状态 / 时间 / 摘要三要素', () => {
    mount(
      historyState({ runs: [record(1, 'completed', '第一跑'), record(2, 'failed', '第二跑')] }),
    );

    const rows = screen.getAllByTestId('agent-run-row');
    expect(rows).toHaveLength(2);
    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['已完成', '失败']);
    expect(rows[0]?.textContent).toContain('第一跑');
    expect(rows[0]?.textContent).toContain('#1');
    expect(rows[0]?.textContent).toContain('4 轮');
    // 时间以本地时区字符串呈现（非空即可，不绑定格式）
    expect((rows[0]?.textContent ?? '').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('run-row-error')).toHaveLength(1);
    expect(screen.getAllByTestId('run-row-error')[0]?.textContent).toContain(
      '进程结束但未产出 result 事件',
    );
  });

  it('status 为 stopped 的 run 行状态标签呈「已停止」，其余三态标签不回归', () => {
    mount(
      historyState({
        runs: [
          record(1, 'running', '运行中跑'),
          record(2, 'completed', '完成跑'),
          record(3, 'stopped', '停止跑'),
          record(4, 'failed', '失败跑'),
        ],
      }),
    );

    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['运行中', '已完成', '已停止', '失败']);
    const stoppedRow = screen
      .getAllByTestId('agent-run-row')
      .find((row) => row.getAttribute('data-status') === 'stopped');
    expect(stoppedRow?.textContent).toContain('停止跑');
  });

  it('点击 run 行 → openRun(id) 恰调用一次（重放触发半边）', () => {
    const openRun = vi.fn();
    mount(historyState({ runs: [record(7, 'completed', '目标 run')], openRun }));

    fireEvent.click(screen.getByTestId('agent-run-row'));

    expect(openRun).toHaveBeenCalledTimes(1);
    expect(openRun).toHaveBeenCalledWith(7);
  });

  it('点击刷新 → refresh() 恰调用一次（D13 显式触发）', () => {
    const refresh = vi.fn();
    mount(historyState({ runs: [record(1, 'completed', 'x')], refresh }));

    fireEvent.click(screen.getByTestId('history-refresh'));

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('selectedRunId 匹配时呈现重放区并还原时间线（与实时流同组件）', () => {
    mount(
      historyState({
        runs: [record(7, 'completed', '已重放')],
        events: [event(0)],
        selectedRunId: 7,
      }),
    );

    const replay = screen.getByTestId('replay-area');
    expect(replay.getAttribute('data-selected-run-id')).toBe('7');
    expect(within(replay).getByTestId('agent-timeline') !== null).toBe(true);
    expect(within(replay).getByTestId('event-run-started') !== null).toBe(true);
  });

  it('重放区限高滚动：max-h-96 overflow-y-auto 容器，内嵌 AgentTimeline 无 props 分叉', () => {
    mount(
      historyState({
        runs: [record(7, 'completed', '限高重放')],
        events: [event(0), event(1)],
        selectedRunId: 7,
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
  it('runs 为空时呈现空态文案不崩', () => {
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
        runs: [record(1, 'completed', '仍在的 run')],
        error: 'db: 事件读取失败',
        refresh,
      }),
    );

    expect(screen.getByTestId('history-error').textContent).toContain('db: 事件读取失败');
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    // 错误态下刷新入口仍可触发
    fireEvent.click(screen.getByTestId('history-refresh'));
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
