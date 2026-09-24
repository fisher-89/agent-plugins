import { Channel, invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';

import { buildExplorePrompt } from '../../../lib/exploreStance';
import type {
  AgentEnvMode,
  AgentEvent,
  AgentPermissionMode,
  AgentRunRecord,
  ExploreRecord,
} from '../../../types/dto';

/** explore 会话来源受控字符串（与 store 侧 RunProvenance 口径一致） */
const EXPLORE_SOURCE = 'explore';

/** 单次发送入参（与调试页表单语义一致：cwd 隐含 workspace root） */
export interface ExploreSendInput {
  prompt: string;
  env: AgentEnvMode;
  permissionMode: AgentPermissionMode;
}

export interface ExploreSessionState {
  /** 会话链（发起序 run 记录，store 单链还原） */
  chain: AgentRunRecord[];
  /** 对话事件：历史链重放 + 实时累积（发送期间 Channel 逐事件追加） */
  events: AgentEvent[];
  /** 链还原 / 重放进行中 */
  loading: boolean;
  /** 本轮运行中（invoke 未 resolve / reject）；运行中重复发送忽略 */
  running: boolean;
  error: string | null;
  /** 发送一条消息：拼 stance、以链尾 sessionId 续话、携带来源三元组 */
  send: (input: ExploreSendInput) => void;
}

/** 链还原 + 逐 run 事件重放（store 链查询收口单点，hook 不拼链） */
async function loadChain(sourceRef: string): Promise<{
  runs: AgentRunRecord[];
  events: AgentEvent[];
}> {
  const runs = await invoke<AgentRunRecord[]>('agent_run_chain', {
    source: EXPLORE_SOURCE,
    sourceRef,
  });
  const replays = await Promise.all(
    runs.map((run) => invoke<AgentEvent[]>('agent_run_events', { runId: run.id })),
  );
  return { runs, events: replays.flat() };
}

/** 历史半边：链还原 + 落库事件重放；appendRun / appendEvent 供实时半边续写 */
function useChainReplay(root: string | null, recordId: number | null) {
  const [chain, setChain] = useState<AgentRunRecord[]>([]);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!root || recordId === null) {
      setChain([]);
      setEvents([]);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    loadChain(String(recordId))
      .then((loaded) => {
        if (cancelled) return;
        setChain(loaded.runs);
        setEvents(loaded.events);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(String(err));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, recordId]);

  const appendRun = useCallback((run: AgentRunRecord) => setChain((prev) => [...prev, run]), []);
  const appendEvent = useCallback((event: AgentEvent) => setEvents((prev) => [...prev, event]), []);
  return { chain, events, loading, error, appendRun, appendEvent };
}

/**
 * 会话链 hook（对话区数据面）：mount / 记录变更 invoke("agent_run_chain")
 * （source="explore" + 该记录 id 的十进制 source_ref）还原链，并逐 run
 * invoke("agent_run_events") 重放落库事件（重开双恢复的对话半边）。send 组装
 * prompt（stance 前导 + 用户输入）、以链尾 run 的 sessionId 作为
 * resume_session_id 续话、parentRunId 取链尾 id、携带来源三元组 invoke
 * ("agent_start")，事件经 Channel 实时累积；run 终态把返回记录追加进链
 * （下次续话以新链尾为准）。运行中防重复发送。
 */
export function useExploreSession(
  root: string | null,
  record: ExploreRecord | null,
): ExploreSessionState {
  const runningRef = useRef(false);
  // 记录经 ref 取值：效果按 record.id 触发，避免清单 refresh 换对象打断实时流
  const recordRef = useRef(record);
  recordRef.current = record;
  const replay = useChainReplay(root, record?.id ?? null);
  const [running, setRunning] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);

  const send = useCallback(
    (input: ExploreSendInput) => {
      const currentRoot = root;
      const current = recordRef.current;
      if (currentRoot === null || current === null || runningRef.current) return;
      const tail = replay.chain.length > 0 ? replay.chain[replay.chain.length - 1] : null;
      const channel = new Channel<AgentEvent>();
      channel.onmessage = replay.appendEvent;
      runningRef.current = true;
      setRunning(true);
      setSendError(null);
      invoke<AgentRunRecord>('agent_start', {
        onEvent: channel,
        root: currentRoot,
        prompt: buildExplorePrompt(input.prompt),
        env: input.env,
        permissionMode: input.permissionMode,
        resumeSessionId: tail?.sessionId ?? null,
        source: EXPLORE_SOURCE,
        sourceRef: String(current.id),
        parentRunId: tail?.id ?? null,
      })
        .then(replay.appendRun) // 链尾推进，下次续话以新链尾为准
        .catch((err: unknown) => setSendError(String(err)))
        .finally(() => {
          runningRef.current = false;
          setRunning(false);
        });
    },
    [root, replay.chain, replay.appendEvent, replay.appendRun],
  );

  return {
    chain: replay.chain,
    events: replay.events,
    loading: replay.loading,
    running,
    error: replay.error ?? sendError,
    send,
  };
}
