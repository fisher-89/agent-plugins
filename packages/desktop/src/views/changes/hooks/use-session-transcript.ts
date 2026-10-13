import { useEffect, useRef, useState } from 'react';

import { eventsToUIMessages, type AgentUIMessage } from '../../../lib/agent-adapter';
import { commands, type AgentEvent, type SessionSummary } from '../../../types/generated/bindings';

export interface UseSessionTranscriptResult {
  messages: AgentUIMessage[];
  running: boolean;
  error: string | null;
  /** 查询时快照三件套（row + stats + turns）：查无会话 / 未开跑角色为 null */
  summary: SessionSummary | null;
}

/** 转录事件装配：重放 + 实时事件按 seq 去重合并、seq 升序。归档链实时面
 * 专用（archive-panel 自身机制，desktop-change-flow-view 零触点红线 accommodation
 * ——change run 面已统一转录库重查，change-flow 链路不再传入）。 */
function assembleTranscript(replay: AgentEvent[], liveEvents: AgentEvent[]): AgentEvent[] {
  const seen = new Set<number>(replay.map((event) => event.seq));
  const merged = [...replay];
  for (const event of liveEvents) {
    if (!seen.has(event.seq)) {
      seen.add(event.seq);
      merged.push(event);
    }
  }
  return merged.sort((a, b) => a.seq - b.seq);
}

/** 库内装载产物 */
interface TranscriptLoad {
  transcript: AgentEvent[];
  running: boolean;
  summary: SessionSummary;
}

/** 直查优先：槽位 id → `sessionDetail` 单查 → 密封转录重放（记录在案的 id
 * 精确寻址，杜绝「同 ref 多会话取最近一条」反查歧义）。 */
async function loadBySessionId(root: string, sessionId: string): Promise<TranscriptLoad | null> {
  const detail = await commands.sessionDetail(root, sessionId);
  if (detail === null) return null;
  const transcript = await commands.agentSessionTranscript(root, sessionId);
  return {
    transcript,
    running: detail.turns.some((turn) => turn.status === 'running'),
    summary: detail,
  };
}

/** 归档实时事件并入（archive-panel 专用面）：以库内重放为底、seq 去重合并
 *（重放未就绪时静默等待；change run 面恒缺省短路——转录库重查为唯一数据源）。 */
function useArchiveLiveMerge({
  sessionId,
  liveEvents,
  replayRef,
  setMessages,
}: {
  sessionId: string | null;
  liveEvents: AgentEvent[] | undefined;
  replayRef: React.RefObject<AgentEvent[] | null>;
  setMessages: React.Dispatch<React.SetStateAction<AgentUIMessage[]>>;
}): void {
  useEffect(() => {
    const base = replayRef.current;
    if (
      sessionId === null ||
      base === null ||
      liveEvents === undefined ||
      liveEvents.length === 0
    ) {
      return;
    }
    setMessages(eventsToUIMessages(assembleTranscript(base, liveEvents)));
  }, [liveEvents, sessionId, replayRef, setMessages]);
}

/**
 * 会话寻址转录（unify-run-state-persistence：change run 实时面统一转录库——
 * change-flow 链路的 liveEvents 缓存退役，会话事件通知经 `refreshKey`（150ms
 * 去抖）触发转录库重查，运行中会话的已流出转录与运行时一致；`liveEvents`
 * 参数保留仅供归档链实时面（archive-panel 自身机制，零触点红线）复用）：
 * `sessionId` 变化即重查（节点选中联动）。
 */
export function useSessionTranscript(params: {
  root: string | null;
  sessionId: string | null;
  /** 变更通知触发的重查键（自增即重查转录库；显式选中联动不依赖本键） */
  refreshKey?: number;
  /** 归档链实时事件流（archive-panel 专用；change run 面恒缺省） */
  liveEvents?: AgentEvent[];
}): UseSessionTranscriptResult {
  const { root, sessionId, refreshKey, liveEvents } = params;
  const [messages, setMessages] = useState<AgentUIMessage[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<SessionSummary | null>(null);
  // 最近一次库内重放事件（归档实时并入的合并底座；ref 直读不触发重渲染）
  const replayRef = useRef<AgentEvent[] | null>(null);

  useEffect(() => {
    // sessionId=null：清空态不发起任何查询（槽位缺席，不虚构会话）
    if (root === null || sessionId === null) {
      replayRef.current = null;
      setMessages([]);
      setRunning(false);
      setError(null);
      setSummary(null);
      return;
    }
    let disposed = false;
    const load = loadBySessionId(root, sessionId);
    load
      .then((result) => {
        if (disposed) return;
        replayRef.current = result?.transcript ?? null;
        setMessages(result === null ? [] : eventsToUIMessages(result.transcript));
        setRunning(result?.running ?? false);
        setSummary(result?.summary ?? null);
      })
      .catch((cause: unknown) => {
        if (!disposed) setError(typeof cause === 'string' ? cause : String(cause));
      });
    return () => {
      disposed = true;
    };
  }, [root, sessionId, refreshKey]);

  useArchiveLiveMerge({ sessionId, liveEvents, replayRef, setMessages });

  return { messages, running, error, summary };
}
