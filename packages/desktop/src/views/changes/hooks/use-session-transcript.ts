/**
 * 会话转录 hook：按 `source="change"` + sourceRef exact-match 反查会话 →
 * `agent_session_transcript` 密封转录重放 → agent-adapter 适配为
 * `AgentUIMessage` → 实时事件按 seq 去重并入。运行中实时 / 收口重放一致
 *（无第二套时间线——渲染面复用 AgentTimeline）。
 */
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

/**
 * sourceRef 反查转录：`sourceRef` 变化即重查（运行步节点选中联动）；
 * `liveEvents` 为该会话的实时事件流（run 状态缓存按 sessionId 过滤后传入），
 * 与库内重放按 seq 归并——重放覆盖落库半边、实时覆盖在途半边，二者并流
 * 无缝。会话仍有 running 轮行时 `running` 置真（面板头部流式状态）。
 */
export function useSessionTranscript(params: {
  root: string | null;
  sourceRef: string | null;
  liveEvents: AgentEvent[];
}): UseSessionTranscriptResult {
  const { root, sourceRef, liveEvents } = params;
  const [messages, setMessages] = useState<AgentUIMessage[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 最近一次库内重放事件（实时并入的合并底座；ref 直读不触发重渲染）
  const replayRef = useRef<AgentEvent[] | null>(null);

  useEffect(() => {
    if (root === null || sourceRef === null) {
      replayRef.current = null;
      setMessages([]);
      setRunning(false);
      setError(null);
      return;
    }
    let disposed = false;
    void commands
      .agentSessions(root, 'change', sourceRef)
      .then(async (summaries) => {
        if (disposed) return;
        if (summaries.length === 0) {
          replayRef.current = null;
          setMessages([]);
          setRunning(false);
          return;
        }
        // 同 sourceRef 多会话（重发同 attempt 的罕见形态）取最近一条
        const latest = summaries[summaries.length - 1];
        setRunning(latest.turns.some((turn) => turn.status === 'running'));
        const transcript = await commands.agentSessionTranscript(root, latest.row.id);
        if (disposed) return;
        replayRef.current = transcript;
        setMessages(eventsToUIMessages(transcript));
      })
      .catch((cause: unknown) => {
        if (!disposed) setError(typeof cause === 'string' ? cause : String(cause));
      });
    return () => {
      disposed = true;
    };
  }, [root, sourceRef]);

  // 实时事件并入：以库内重放为底、seq 去重合并（重放未就绪时静默等待）
  useEffect(() => {
    const base = replayRef.current;
    if (sourceRef === null || base === null || liveEvents.length === 0) return;
    setMessages(eventsToUIMessages(assembleTranscript(base, liveEvents)));
  }, [liveEvents, sourceRef]);

  return { messages, running, error };
}
