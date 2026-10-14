import { useCallback, useEffect, useState } from 'react';

import { commands, type AgentEvent, type SessionSummary } from '../../../types/generated/bindings';

/** 历史区来源筛选三态：'all' 在取数层映射 null（不过滤）。 */
export type AgentHistorySource = 'debug' | 'change' | 'all';

/** 每页条数（固定常量，UI 不暴露设置项）。 */
const PAGE_SIZE = 20;

export interface AgentRunHistoryState {
  /** 当前页会话清单（后端 `updated_at` 降序）；挂载自动取数，refresh() 重取 */
  sessions: SessionSummary[];
  /** 点开重放的密封事件流（seq 升序） */
  events: AgentEvent[];
  /** 当前重放的会话 id；null 即未点开 */
  selectedSessionId: string | null;
  loading: boolean;
  error: string | null;
  /** 过滤集全量计数（接口 `total` 快照；非当前页长度） */
  total: number;
  /** 当前页码（1 起） */
  page: number;
  /** 总页数（`max(1, ceil(total / PAGE_SIZE))` 导出） */
  totalPages: number;
  /** 显式刷新会话清单（停留在当前页与当前筛选） */
  refresh: () => void;
  /** 跳页（夹在 `[1, totalPages]`，越界不动） */
  setPage: (page: number) => void;
  /** 下一页（末页时不动） */
  nextPage: () => void;
  /** 上一页（首页时不动） */
  prevPage: () => void;
  /** 点开一条会话：invoke("agent_session_transcript") 重放全史密封转录 */
  openSession: (sessionId: string) => void;
}

interface QueryState {
  loading: boolean;
  error: string | null;
}

const IDLE: QueryState = { loading: false, error: null };

/** 总页数（`total` 为过滤集全量计数；空清单兜底单页，页界不塌 0）。 */
function maxPageOf(total: number): number {
  return Math.max(1, Math.ceil(total / PAGE_SIZE));
}

/** 夹紧页码：`[1, maxPageOf(total)]`。 */
function clampPage(next: number, total: number): number {
  return Math.min(Math.max(1, next), maxPageOf(total));
}

/** 清单分页态：总量 + 页码（夹紧跳页；root / source 变化回第 1 页）。 */
function useSessionPaging(root: string | null, source: AgentHistorySource) {
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);

  const gotoPage = useCallback((next: number) => setPage(clampPage(next, total)), [total]);

  // root / source 变化回第 1 页（已在第 1 页时 React 状态 bailout，零额外 invoke）
  useEffect(() => {
    setPage(1);
  }, [root, source]);

  return {
    total,
    setTotal,
    page,
    setPage,
    totalPages: maxPageOf(total),
    gotoPage,
  };
}

interface SessionsView {
  sessions: SessionSummary[];
  total: number;
  page: number;
  totalPages: number;
  query: QueryState;
  refresh: () => void;
  setPage: (page: number) => void;
}

function useSessions(root: string | null, source: AgentHistorySource): SessionsView {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [query, setQuery] = useState<QueryState>(IDLE);
  const [tick, setTick] = useState(0);
  const paging = useSessionPaging(root, source);
  const { total, page, totalPages } = paging;

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (root === null) {
      setSessions([]);
      paging.setTotal(0);
      paging.setPage(1);
      setQuery(IDLE);
      return;
    }
    let cancelled = false;
    setQuery({ loading: true, error: null });
    commands
      .agentSessions(
        root,
        source === 'all' ? null : source,
        null,
        (page - 1) * PAGE_SIZE,
        PAGE_SIZE,
      )
      .then((result) => {
        if (cancelled) return;
        // 总量收缩致当前页越界：回末页重取，本页空结果不落（loading 保持到落定）。
        // 此处用 raw setPage（目标页已算得合法值），避开 gotoPage 闭包里的旧 total
        paging.setTotal(result.total);
        const maxPage = maxPageOf(result.total);
        if (page > maxPage) {
          paging.setPage(maxPage);
          return;
        }
        setSessions(result.sessions);
        setQuery(IDLE);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setQuery({ loading: false, error: String(err) });
      });
    return () => {
      cancelled = true;
    };
  }, [root, source, page, tick]);

  return { sessions, total, page, totalPages, query, refresh, setPage: paging.gotoPage };
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
 * 历史会话 hook：`source` 筛选会话来源（默认由调用方持 'debug'），切换即重查
 * 并回第 1 页；清单按 `PAGE_SIZE` 分页（`total` 为过滤集全量计数，页界由其导
 * 出）；显式刷新（tick 重取）停留当前页，无轮询无订阅。
 */
export function useAgentRunHistory(
  root: string | null,
  source: AgentHistorySource,
): AgentRunHistoryState {
  const sessionsState = useSessions(root, source);
  const replayState = useReplay(root);
  return {
    sessions: sessionsState.sessions,
    events: replayState.events,
    selectedSessionId: replayState.selectedSessionId,
    loading: sessionsState.query.loading || replayState.query.loading,
    error: sessionsState.query.error ?? replayState.query.error,
    total: sessionsState.total,
    page: sessionsState.page,
    totalPages: sessionsState.totalPages,
    refresh: sessionsState.refresh,
    setPage: sessionsState.setPage,
    nextPage: () => sessionsState.setPage(sessionsState.page + 1),
    prevPage: () => sessionsState.setPage(sessionsState.page - 1),
    openSession: replayState.openSession,
  };
}
