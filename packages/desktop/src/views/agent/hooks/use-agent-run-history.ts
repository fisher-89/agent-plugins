import { useCallback, useEffect, useState } from 'react';

import { commands, type AgentEvent, type AgentRunRecord } from '../../../types/generated/bindings';

export interface AgentRunHistoryState {
  /** 运行清单（后端 started_at 降序）；挂载自动取数，refresh() 重取 */
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
 * 运行清单轨道：挂载即自动取数（tick 从 0 起即触发 invoke("agent_runs")，
 * 携 root 寻址当前 workspace 库——清单收窄为本 workspace 历史），刷新仍经
 * tick 递增（refresh()）。root 为 null（未选定 workspace）跳过 invoke 保持
 * 空态。无轮询、无文件 watch。
 */
function useRuns(root: string | null): {
  runs: AgentRunRecord[];
  query: QueryState;
  refresh: () => void;
} {
  const [runs, setRuns] = useState<AgentRunRecord[]>([]);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (root === null) {
      setRuns([]);
      setQuery(IDLE);
      return;
    }
    let cancelled = false;
    setQuery({ loading: true, error: null });
    commands
      .agentRuns(root)
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
  }, [root, tick]);

  return { runs, query, refresh };
}

/**
 * 重放轨道：点开 run 触发 invoke("agent_run_events")（携 root 寻址所属
 * workspace 库），以落库事件还原时间线（不要求原运行进程存活）；root 为
 * null 跳过 invoke 保持空态。
 */
function useReplay(root: string | null): {
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
    if (target === null || root === null) return;
    let cancelled = false;
    setQuery({ loading: true, error: null });
    commands
      .agentRunEvents(root, target)
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
  }, [root, target]);

  return { events, selectedRunId, query, openRun };
}

/**
 * 历史运行 hook：挂载即自动取当前 workspace 的运行清单（root 寻址；null 跳
 * 过取数保持空态），refresh() 显式重取，openRun(id) 重放事件；run 结束不自
 * 动刷新（design：查询取数显式触发，Channel 例外不外溢）、无轮询。
 */
export function useAgentRunHistory(root: string | null): AgentRunHistoryState {
  const runsState = useRuns(root);
  const replayState = useReplay(root);
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
