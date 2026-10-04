import { useEffect, useRef, useState } from 'react';

import { eventsToUIMessages, type AgentUIMessage } from '../../../lib/agent-adapter';
import { commands, type AgentEvent } from '../../../types/generated/bindings';

export interface UseSessionTranscriptResult {
  messages: AgentUIMessage[];
  running: boolean;
  error: string | null;
}

/** 转录事件装配：重放 + 实时事件按 seq 去重合并、seq 升序。 */
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

/** 库内装载产物：密封转录重放 + 运行状态（自轮行推导——有 running 轮行即
 * running）；`null` 即空态（查无会话 / blank root）。 */
interface TranscriptLoad {
  transcript: AgentEvent[];
  running: boolean;
}

/** 直查优先：槽位 id → `sessionDetail` 单查 → 密封转录重放（记录在案的 id
 * 精确寻址，杜绝「同 ref 多会话取最近一条」反查歧义）。 */
async function loadBySessionId(root: string, sessionId: string): Promise<TranscriptLoad | null> {
  const detail = await commands.sessionDetail(root, sessionId);
  if (detail === null) return null;
  const transcript = await commands.agentSessionTranscript(root, sessionId);
  return { transcript, running: detail.turns.some((turn) => turn.status === 'running') };
}

/** 反查兜底：sourceRef 定式 exact-match（旧数据，行为与升级前一致；同
 * sourceRef 多会话取最近一条）。 */
async function loadBySourceRef(root: string, sourceRef: string): Promise<TranscriptLoad | null> {
  const summaries = await commands.agentSessions(root, 'change', sourceRef);
  if (summaries.length === 0) return null;
  const latest = summaries[summaries.length - 1];
  const transcript = await commands.agentSessionTranscript(root, latest.row.id);
  return { transcript, running: latest.turns.some((turn) => turn.status === 'running') };
}

/**
 * 会话寻址转录：`sessionId` / `sourceRef` 任一变化即重查（节点选中联动）；
 * `liveEvents` 为该会话的实时事件流（run 状态缓存按 sessionId 过滤后传入），
 * 与库内重放按 seq 归并——重放覆盖落库半边、实时覆盖在途半边，二者并流
 * 无缝。
 */
export function useSessionTranscript(params: {
  root: string | null;
  sourceRef: string | null;
  sessionId: string | null;
  liveEvents: AgentEvent[];
}): UseSessionTranscriptResult {
  const { root, sourceRef, sessionId, liveEvents } = params;
  const [messages, setMessages] = useState<AgentUIMessage[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 最近一次库内重放事件（实时并入的合并底座；ref 直读不触发重渲染）
  const replayRef = useRef<AgentEvent[] | null>(null);

  useEffect(() => {
    // 双 null：清空态不发起任何查询（槽位缺席且无反查键，不虚构会话）
    if (root === null || (sessionId === null && sourceRef === null)) {
      replayRef.current = null;
      setMessages([]);
      setRunning(false);
      setError(null);
      return;
    }
    let disposed = false;
    const load = (async (): Promise<TranscriptLoad | null> => {
      if (sessionId !== null) return loadBySessionId(root, sessionId);
      if (sourceRef !== null) return loadBySourceRef(root, sourceRef);
      return null;
    })();
    load
      .then((result) => {
        if (disposed) return;
        replayRef.current = result?.transcript ?? null;
        setMessages(result === null ? [] : eventsToUIMessages(result.transcript));
        setRunning(result?.running ?? false);
      })
      .catch((cause: unknown) => {
        if (!disposed) setError(typeof cause === 'string' ? cause : String(cause));
      });
    return () => {
      disposed = true;
    };
  }, [root, sourceRef, sessionId]);

  // 实时事件并入：以库内重放为底、seq 去重合并（重放未就绪时静默等待）
  useEffect(() => {
    const key = sessionId ?? sourceRef;
    const base = replayRef.current;
    if (key === null || base === null || liveEvents.length === 0) return;
    setMessages(eventsToUIMessages(assembleTranscript(base, liveEvents)));
  }, [liveEvents, sessionId, sourceRef]);

  return { messages, running, error };
}
