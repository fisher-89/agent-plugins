import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../../../types/dto';
import { useAgentRunHistory } from './useAgentRunHistory';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_runs / agent_run_events）。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

function record(id: number, startedAt: number, status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id,
    prompt: `任务 ${id}`,
    cwd: 'C:\\demo\\beta',
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt,
    finishedAt: startedAt + 500,
    numTurns: 2,
    costUsd: 0.1,
    durationMs: 500,
    sessionId: `s-${id}`,
    error: null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
  };
}

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-9',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery"}',
  };
}

/** 后端清单形态：startedAt 降序（id 3 最近）。 */
const RUNS_DESC = [record(3, 300, 'completed'), record(2, 100, 'failed')];

describe('useAgentRunHistory：显式刷新与点开重放', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('挂载不自动取数：不发起任何 invoke', () => {
    renderHook(() => useAgentRunHistory());

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('refresh() → invoke("agent_runs") → runs 按后端降序原样承接', async () => {
    invokeMock.mockResolvedValue(RUNS_DESC);
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.runs).toEqual(RUNS_DESC));

    expect(invokeMock).toHaveBeenCalledWith('agent_runs');
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('openRun(id) → invoke("agent_run_events", { runId }) → events 更新、selectedRunId 跟随', async () => {
    invokeMock.mockResolvedValue([]);
    const events = [runStarted(0), raw(1)];
    invokeMock.mockImplementation((command: string, params?: { runId?: number }) => {
      if (command === 'agent_run_events') {
        return Promise.resolve(params?.runId === 7 ? events : []);
      }
      return Promise.resolve([]);
    });
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.openRun(7);
    });
    await waitFor(() => expect(result.current.events).toEqual(events));

    expect(invokeMock).toHaveBeenCalledWith('agent_run_events', { runId: 7 });
    expect(result.current.selectedRunId).toBe(7);
  });

  it('refresh reject：error 置串、loading 复位（显式触发失败不静默）', async () => {
    invokeMock.mockRejectedValue('db: 清单打开失败');
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toBe('db: 清单打开失败'));

    expect(result.current.loading).toBe(false);
  });

  it('openRun reject：error 置串、loading 复位、runs 清单不受影响', async () => {
    invokeMock.mockResolvedValue(RUNS_DESC);
    const { result } = renderHook(() => useAgentRunHistory());
    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.runs).toEqual(RUNS_DESC));

    invokeMock.mockRejectedValue('db: 事件读取失败');
    act(() => {
      result.current.openRun(7);
    });
    await waitFor(() => expect(result.current.error).toBe('db: 事件读取失败'));

    expect(result.current.loading).toBe(false);
    expect(result.current.runs).toEqual(RUNS_DESC);
  });

  it('取数进行中 loading=true、返回后复位；两轨道任一取数中即 loading', async () => {
    let resolveRuns: (runs: AgentRunRecord[]) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise<AgentRunRecord[]>((resolve) => {
          resolveRuns = resolve;
        }),
    );
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    await act(async () => {
      resolveRuns(RUNS_DESC);
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('agent_runs 返回 []：runs 为空数组不崩', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.refresh();
    });
    // loading 复位的状态更新可能晚于 runs 到达：三断言一并置于 waitFor 内轮询
    await waitFor(() => {
      expect(result.current.runs).toEqual([]);
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);
    });
  });

  it('连续 openRun 不同 id：events 以最后一次为准、selectedRunId 跟随', async () => {
    const eventsFirst = [runStarted(0)];
    const eventsSecond = [raw(0)];
    invokeMock.mockImplementation((command: string, params?: { runId?: number }) => {
      if (command === 'agent_run_events') {
        return Promise.resolve(params?.runId === 1 ? eventsFirst : eventsSecond);
      }
      return Promise.resolve([]);
    });
    const { result } = renderHook(() => useAgentRunHistory());

    act(() => {
      result.current.openRun(1);
    });
    await waitFor(() => expect(result.current.selectedRunId).toBe(1));

    act(() => {
      result.current.openRun(2);
    });
    await waitFor(() => expect(result.current.selectedRunId).toBe(2));
    await waitFor(() => expect(result.current.events).toEqual(eventsSecond));
  });
});
