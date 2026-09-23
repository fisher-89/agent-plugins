import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';

import type { AgentEvent, AgentRunRecord } from '../../../types/dto';

export interface AgentRunHistoryState {
  /** 运行清单（后端 started_at 降序）；仅显式 refresh 后取得 */
  runs: AgentRunRecord[];
  /** 点开重放的事件流（seq 升序） */
  events: AgentEvent[];
  /** 当前重放的 run id；null 即未点开 */
  selectedRunId: number | null;
  loading: boolean;
  error: string | null;
  /** 显式刷新运行清单 */
  refresh: () => void;
  /** 点开一条 run：invoke("agent_run_events") 重放落库事件 */
  openRun: (runId: number) => void;
}

interface QueryState {
  loading: boolean;
  error: string | null;
}

const IDLE: QueryState = { loading: false, error: null };

/**
 * 运行清单轨道：挂载不自动取数（tick===0 跳过），仅显式 refresh 触发
 * invoke("agent_runs")。无轮询、无文件 watch。
 */
function useRuns(): { runs: AgentRunRecord[]; query: QueryState; refresh: () => void } {
  const [runs, setRuns] = useState<AgentRunRecord[]>([]);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (tick === 0) return;
    let cancelled = false;
    setQuery({ loading: true, error: null });
    invoke<AgentRunRecord[]>('agent_runs')
      .then((result) => {
        if (cancelled) return;
        setRuns(result);
        setQuery(IDLE);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setQuery({ loading: false, error: String(err) });
      });
    return () => {
      cancelled = true;
    };
  }, [tick]);

  return { runs, query, refresh };
}

/**
 * 重放轨道：点开 run 触发 invoke("agent_run_events")，以落库事件还原时间线
 * （不要求原运行进程存活）。
 */
function useReplay(): {
  events: AgentEvent[];
  selectedRunId: number | null;
  query: QueryState;
  openRun: (runId: number) => void;
} {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [target, setTarget] = useState<number | null>(null);

  const openRun = useCallback((runId: number) => setTarget(runId), []);

  useEffect(() => {
    if (target === null) return;
    let cancelled = false;
    setQuery({ loading: true, error: null });
    invoke<AgentEvent[]>('agent_run_events', { runId: target })
      .then((result) => {
        if (cancelled) return;
        setEvents(result);
        setSelectedRunId(target);
        setQuery(IDLE);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setQuery({ loading: false, error: String(err) });
      });
    return () => {
      cancelled = true;
    };
  }, [target]);

  return { events, selectedRunId, query, openRun };
}

/**
 * 历史运行 hook：全部取数由用户显式动作触发——refresh() 取运行清单，
 * openRun(id) 重放事件；挂载不自动取数、run 结束不自动刷新（design：查询
 * 取数显式触发，Channel 例外不外溢）、无轮询。
 */
export function useAgentRunHistory(): AgentRunHistoryState {
  const runsState = useRuns();
  const replayState = useReplay();
  return {
    runs: runsState.runs,
    events: replayState.events,
    selectedRunId: replayState.selectedRunId,
    loading: runsState.query.loading || replayState.query.loading,
    error: runsState.query.error ?? replayState.query.error,
    refresh: runsState.refresh,
    openRun: replayState.openRun,
  };
}
