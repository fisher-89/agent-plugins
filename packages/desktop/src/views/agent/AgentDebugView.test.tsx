// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../../types/dto';
import { AgentDebugView } from './AgentDebugView';

// ---------------------------------------------------------------------------
// agent 域 hooks 以 vi.mock 返回受控 fixture state（组件纯 props 驱动）。
// ---------------------------------------------------------------------------

const { useAgentRunMock, useAgentRunHistoryMock } = vi.hoisted(() => ({
  useAgentRunMock: vi.fn(),
  useAgentRunHistoryMock: vi.fn(),
}));

vi.mock('./hooks/useAgentRun', () => ({ useAgentRun: useAgentRunMock }));
vi.mock('./hooks/useAgentRunHistory', () => ({ useAgentRunHistory: useAgentRunHistoryMock }));

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function resultRecord(status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id: 1,
    prompt: '你好',
    cwd: 'C:\\demo\\beta',
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000,
    finishedAt: 1727000001000,
    numTurns: 3,
    costUsd: 0.5,
    durationMs: 1234,
    sessionId: 's-1',
    error: null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
  };
}

const IDLE_RUN = {
  events: [] as AgentEvent[],
  running: false,
  error: null,
  result: null,
  start: vi.fn(),
};

const IDLE_HISTORY = {
  runs: [],
  events: [],
  selectedRunId: null,
  loading: false,
  error: null,
  refresh: vi.fn(),
  openRun: vi.fn(),
};

describe('AgentDebugView：页面骨架组装（AC-5）', () => {
  beforeEach(() => {
    useAgentRunMock.mockReset().mockReturnValue({ ...IDLE_RUN, start: vi.fn() });
    useAgentRunHistoryMock.mockReset().mockReturnValue({ ...IDLE_HISTORY, refresh: vi.fn() });
  });

  it('渲染四区块：参数面、事件时间线、原始 JSONL 切换入口、历史运行区', () => {
    render(<AgentDebugView root="C:\\demo\\beta" />);

    expect(screen.getByTestId('agent-run-form') !== null).toBe(true);
    expect(screen.getByTestId('agent-timeline') !== null).toBe(true);
    expect(screen.getByTestId('stream-toggle') !== null).toBe(true);
    expect(screen.getByTestId('agent-run-history') !== null).toBe(true);
  });

  it('原始 JSONL 切换：切至 AgentRawStream 呈现、切回时间线（D12 数据源为事件落库形态）', () => {
    useAgentRunMock.mockReturnValue({ ...IDLE_RUN, events: [runStarted(0)] });
    render(<AgentDebugView root="C:\\demo\\beta" />);

    expect(screen.getByTestId('agent-timeline') !== null).toBe(true);
    fireEvent.click(screen.getByTestId('toggle-raw'));
    expect(screen.getByTestId('agent-raw-stream') !== null).toBe(true);
    expect(screen.queryByTestId('agent-timeline')).toBeNull();

    fireEvent.click(screen.getByTestId('toggle-timeline'));
    expect(screen.getByTestId('agent-timeline') !== null).toBe(true);
    expect(screen.queryByTestId('agent-raw-stream')).toBeNull();
  });

  it('root=null 时启动入口缺席：页面自呈现提示、不崩', () => {
    render(<AgentDebugView root={null} />);

    expect(screen.getByTestId('agent-no-root') !== null).toBe(true);
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.queryByTestId('agent-start')).toBeNull();
  });

  it('running 中表单禁用（disabled 向 AgentRunForm 传导）', () => {
    useAgentRunMock.mockReturnValue({ ...IDLE_RUN, running: true });
    render(<AgentDebugView root="C:\\demo\\beta" />);

    expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(true);
  });

  it('MVP 边界（AC-6）：全页无停止 / 取消 / kill 入口', () => {
    render(<AgentDebugView root="C:\\demo\\beta" />);

    const killish = screen.queryAllByText(/(停止|取消|终止|中断|kill)/i);
    expect(killish).toHaveLength(0);
  });

  it('运行失败（error 非空）呈现错误条', () => {
    useAgentRunMock.mockReturnValue({ ...IDLE_RUN, error: 'CLI 未找到' });
    render(<AgentDebugView root="C:\\demo\\beta" />);

    expect(screen.getByTestId('run-error').textContent).toContain('CLI 未找到');
  });

  it('run 已结束呈现最终记录状态（completed 结果注入时间线渲染）', () => {
    useAgentRunMock.mockReturnValue({
      ...IDLE_RUN,
      events: [
        runStarted(0),
        {
          seq: 1,
          timestampMs: 1727000000001,
          kind: 'runResult',
          subtype: 'success',
          isError: false,
          numTurns: 3,
          durationMs: 1234,
          costUsd: 0.5,
          usage: {},
          sessionId: 's-1',
        },
      ],
      result: resultRecord('completed'),
    });
    render(<AgentDebugView root="C:\\demo\\beta" />);

    expect(screen.getByTestId('event-result') !== null).toBe(true);
    expect(screen.getByTestId('result-num-turns').textContent).toBe('3');
  });
});
