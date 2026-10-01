import { useCallback, useEffect, useState } from 'react';

import { commands, type AgentEvent, type SessionSummary } from '../../../types/generated/bindings';

export interface AgentRunHistoryState {
  /** 会话清单（后端 `updated_at` 降序）；挂载自动取数，refresh() 重取 */
  sessions: SessionSummary[];
  /** 点开重放的密封事件流（seq 升序） */
  events: AgentEvent[];
  /** 当前重放的会话 id；null 即未点开 */
  selectedSessionId: string | null;
  loading: boolean;
  error: string | null;
  /** 显式刷新会话清单 */
  refresh: () => void;
  /** 点开一条会话：invoke("agent_session_transcript") 重放全史密封转录 */
  openSession: (sessionId: string) => void;
}

interface QueryState {
  loading: boolean;
  error: string | null;
}

const IDLE: QueryState = { loading: false, error: null };

function useSessions(root: string | null): {
  sessions: SessionSummary[];
  query: QueryState;
  refresh: () => void;
} {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (root === null) {
      setSessions([]);
      setQuery(IDLE);
      return;
    }
    let cancelled = false;
    setQuery({ loading: true, error: null });
    commands
      .agentSessions(root, 'debug', null)
      .then((result) => {
        if (cancelled) return;
        setSessions(result);
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

  return { sessions, query, refresh };
}

function useReplay(root: string | null): {
  events: AgentEvent[];
  selectedSessionId: string | null;
  query: QueryState;
  openSession: (sessionId: string) => void;
} {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [target, setTarget] = useState<string | null>(null);

  const openSession = useCallback((sessionId: string) => setTarget(sessionId), []);

  useEffect(() => {
    if (target === null || root === null) return;
    let cancelled = false;
    setQuery({ loading: true, error: null });
    commands
      .agentSessionTranscript(root, target)
      .then((result) => {
        if (cancelled) return;
        setEvents(result);
        setSelectedSessionId(target);
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

  return { events, selectedSessionId, query, openSession };
}

/**
 * 历史会话 hook
 */
export function useAgentRunHistory(root: string | null): AgentRunHistoryState {
  const sessionsState = useSessions(root);
  const replayState = useReplay(root);
  return {
    sessions: sessionsState.sessions,
    events: replayState.events,
    selectedSessionId: replayState.selectedSessionId,
    loading: sessionsState.query.loading || replayState.query.loading,
    error: sessionsState.query.error ?? replayState.query.error,
    refresh: sessionsState.refresh,
    openSession: replayState.openSession,
  };
}
