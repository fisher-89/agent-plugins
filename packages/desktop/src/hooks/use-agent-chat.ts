import { useChat } from '@ai-sdk/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  eventsToUIMessages,
  runRecordToUIMessage,
  type AgentUIMessage,
} from '../lib/agent-adapter';
import { TauriAgentTransport } from '../lib/agent-transport';
import {
  commands,
  type AgentEvent,
  type AgentPermissionMode,
  type SessionSummary,
  type TurnSummary,
} from '../types/generated/bindings';

/** 会话来源参数（source 二元组 + cwd；sourceRef=null 即不重放装载） */
export interface UseAgentChatParams {
  source: string;
  sourceRef: string | null;
  root: string | null;
}

/** 单次发送入参 */
interface AgentChatSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
  agent?: number | null;
}

/** 会话基建状态面（镜像 + 行为；类型经本面结构化流转，视图层不 import ai） */
export interface UseAgentChatState {
  messages: AgentUIMessage[];
  events: AgentEvent[];
  /** 当前会话镜像（重放装载为最新会话；运行中为 early-resolve 回写的会话） */
  session: SessionSummary | null;
  /** 轮统计行列表（发起顺序；运行中含 running 行） */
  chain: TurnSummary[];
  loading: boolean;
  running: boolean;
  error: string | null;
  sendMessage: (input: AgentChatSendInput) => void;
  stop: () => void;
  reset: () => void;
}

/**
 * 重放/归一重建：密封转录折叠 + 轮统计行部件交错（每个 TurnDone 事件位后
 * 插入对应轮行，按轮序对齐；无 TurnDone 的轮行尾部补齐——停止收敛等）。
 */
function buildMessages(events: AgentEvent[], turns: TurnSummary[]): AgentUIMessage[] {
  const replayed = eventsToUIMessages(events);
  const messages: AgentUIMessage[] = [];
  let turnIndex = 0;
  events.forEach((event, index) => {
    messages.push(replayed[index]);
    if (event.kind === 'turnDone' && turnIndex < turns.length) {
      messages.push(runRecordToUIMessage(turns[turnIndex]));
      turnIndex += 1;
    }
  });
  for (; turnIndex < turns.length; turnIndex += 1) {
    messages.push(runRecordToUIMessage(turns[turnIndex]));
  }
  return messages;
}

/** 会话镜像与轮行的会话域同步（row 不变、stats/turns 现算） */
function withTurns(session: SessionSummary, turns: TurnSummary[]): SessionSummary {
  return { ...session, stats: { ...session.stats, turnCount: turns.length }, turns };
}

/** 运行中轮行的会话镜像（early-resolve 回写；字段以轮行为准的运行时投影） */
function sessionFromTurn(
  record: TurnSummary,
  source: string,
  sourceRef: string | null,
): SessionSummary {
  return {
    row: {
      id: record.sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source, sourceRef },
      createdAt: record.startedAt,
      updatedAt: record.startedAt,
    },
    stats: { turnCount: 0, totalDurationMs: null, inputTokens: null, outputTokens: null },
    turns: [],
  };
}

/** 镜像内部态（ref 真相 + state 渲染双轨，transport 回调与发送组装读 ref） */
interface SessionMirrors {
  session: SessionSummary | null;
  turns: TurnSummary[];
  events: AgentEvent[];
  sessionRef: React.RefObject<SessionSummary | null>;
  turnsRef: React.RefObject<TurnSummary[]>;
  eventsRef: React.RefObject<AgentEvent[]>;
  currentSessionIdRef: React.RefObject<string | null>;
  handleEvent: (event: AgentEvent) => void;
  handleRecord: (record: TurnSummary) => void;
  setSession: (session: SessionSummary | null) => void;
  setTurns: (turns: TurnSummary[]) => void;
  setEvents: (events: AgentEvent[]) => void;
  clear: () => void;
}

/** 轮行写入回调装配（upsert / 终态替换 / 清空）：纯回调，不持有状态 */
function chainWrites(
  sessionRef: React.RefObject<SessionSummary | null>,
  turnsRef: React.RefObject<TurnSummary[]>,
  eventsRef: React.RefObject<AgentEvent[]>,
  currentSessionIdRef: React.RefObject<string | null>,
  setSession: (session: SessionSummary | null) => void,
  setTurns: (turns: TurnSummary[]) => void,
  setEvents: (events: AgentEvent[]) => void,
  source: string,
  sourceRef: string | null,
): {
  upsertTurn: (record: TurnSummary) => void;
  handleRecord: (record: TurnSummary) => void;
  clear: () => void;
} {
  const upsertTurn = (record: TurnSummary) => {
    const next = [...turnsRef.current];
    const index = next.findIndex((turn) => turn.turnId === record.turnId);
    if (index >= 0) next[index] = record;
    else next.push(record);
    turnsRef.current = next;
    setTurns(next);
    const current = sessionRef.current;
    if (current !== null) {
      const synced = withTurns(current, next);
      sessionRef.current = synced;
      setSession(synced);
    }
  };
  // transport 观测点：early-resolve running 轮行（回写当前会话 id）/ 终态
  // 轮行按轮 id 替换
  const handleRecord = (record: TurnSummary) => {
    currentSessionIdRef.current = record.sessionId;
    if (sessionRef.current?.row.id !== record.sessionId) {
      const mirror = sessionFromTurn(record, source, sourceRef);
      sessionRef.current = mirror;
      setSession(mirror);
    }
    upsertTurn(record);
  };
  const clear = () => {
    sessionRef.current = null;
    turnsRef.current = [];
    eventsRef.current = [];
    currentSessionIdRef.current = null;
    setSession(null);
    setTurns([]);
    setEvents([]);
  };
  return { upsertTurn, handleRecord, clear };
}

/** 镜像与 transport 观测回调：running 轮行回写会话、终态按轮 id 替换、密封
 * 事件入镜像（delta 只上 chunk 流不入镜像） */
function useSessionMirrors(source: string, sourceRef: string | null): SessionMirrors {
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [turns, setTurns] = useState<TurnSummary[]>([]);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const sessionRef = useRef<SessionSummary | null>(null);
  const turnsRef = useRef<TurnSummary[]>([]);
  const eventsRef = useRef<AgentEvent[]>([]);
  const currentSessionIdRef = useRef<string | null>(null);
  const { handleRecord, clear } = chainWrites(
    sessionRef,
    turnsRef,
    eventsRef,
    currentSessionIdRef,
    setSession,
    setTurns,
    setEvents,
    source,
    sourceRef,
  );

  // transport 观测点：实时密封事件入镜像（append 保序）
  const handleEvent = useCallback((event: AgentEvent) => {
    if (event.kind === 'messageDelta') return; // delta 仅实时流可见
    const next = [...eventsRef.current, event];
    eventsRef.current = next;
    setEvents(next);
  }, []);

  return {
    session,
    turns,
    events,
    sessionRef,
    turnsRef,
    eventsRef,
    currentSessionIdRef,
    handleEvent,
    handleRecord,
    setSession,
    setTurns,
    setEvents,
    clear,
  };
}

/** 重放取数：最新会话（`updated_at` 降序首项）+ 其全史密封转录（空会话空转录） */
async function loadReplay(
  root: string,
  source: string,
  sourceRef: string,
): Promise<{ latest: SessionSummary | null; events: AgentEvent[] }> {
  const sessions = await commands.agentSessions(root, source, sourceRef);
  const latest = sessions[0] ?? null;
  const events = latest !== null ? await commands.agentSessionTranscript(root, latest.row.id) : [];
  return { latest, events };
}

/** 重放结果回写：镜像真相（ref）与渲染态、消息重建一并落位 */
function applyReplay(
  mirrors: SessionMirrors,
  setMessages: (messages: AgentUIMessage[]) => void,
  latest: SessionSummary | null,
  events: AgentEvent[],
): void {
  mirrors.sessionRef.current = latest;
  mirrors.turnsRef.current = latest?.turns ?? [];
  mirrors.eventsRef.current = events;
  mirrors.currentSessionIdRef.current = latest?.row.id ?? null;
  mirrors.setSession(latest);
  mirrors.setTurns(latest?.turns ?? []);
  mirrors.setEvents(events);
  setMessages(buildMessages(events, latest?.turns ?? []));
}

/** 重放装载：source 二元组变更触发；镜像与消息状态按查询结果重建 */
function useSessionReplay(
  params: UseAgentChatParams,
  mirrorsRef: React.RefObject<SessionMirrors>,
  setMessages: (messages: AgentUIMessage[]) => void,
): { loading: boolean; error: string | null } {
  const { source, sourceRef, root } = params;
  const [loading, setLoading] = useState(false);
  const [replayError, setReplayError] = useState<string | null>(null);

  useEffect(() => {
    // 镜像经 ref 读取：效果仅随 source 二元组触发（镜像对象每渲染重建，
    // 直接入 deps 会造成逐渲染重放循环）
    const mirrors = mirrorsRef.current;
    if (root === null || sourceRef === null) {
      mirrors.clear();
      setLoading(false);
      setReplayError(null);
      setMessages([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setReplayError(null);
    loadReplay(root, source, sourceRef)
      .then(({ latest, events }) => {
        if (cancelled) return;
        applyReplay(mirrors, setMessages, latest, events);
        setLoading(false);
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        setReplayError(String(cause));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [source, sourceRef, root, setMessages, mirrorsRef]);

  return { loading, error: replayError };
}

/** 会话行为（发送组装 / 停止 / 重置）；会话参数在此收口、body 穿透 transport */
function useSessionActions(
  chat: ReturnType<typeof useChat<AgentUIMessage>>,
  mirrors: SessionMirrors,
  params: UseAgentChatParams,
  setMessages: (messages: AgentUIMessage[]) => void,
): {
  sendMessage: (input: AgentChatSendInput) => void;
  stop: () => void;
  reset: () => void;
} {
  const { source, sourceRef, root } = params;

  const sendMessage = useCallback(
    (input: AgentChatSendInput) => {
      if (root === null) return;
      // 运行中重复发送忽略（status 直读 chat 实例，同步无竞态）
      if (chat.status === 'submitted' || chat.status === 'streaming') return;
      void chat.sendMessage(undefined, {
        body: {
          root,
          prompt: input.prompt,
          permissionMode: input.permissionMode,
          agent: input.agent ?? null,
          // 当前会话 Continue（early-resolve 轮行回写）；无会话 New
          sessionId: mirrors.currentSessionIdRef.current,
          source,
          sourceRef,
        },
      });
    },
    [chat, mirrors.currentSessionIdRef, root, source, sourceRef],
  );

  const stop = useCallback(() => {
    const sessionId = mirrors.currentSessionIdRef.current;
    // 不调 chat.stop() 截断前端流：终态 record 部件与 finish 由后端闭流推入；
    // 停止寻址携 root（会话 id 即寻址键，root 寻址保留）
    if (sessionId !== null && root !== null) void commands.agentStop(root, sessionId);
  }, [mirrors.currentSessionIdRef, root]);

  const reset = useCallback(() => {
    mirrors.clear();
    setMessages([]);
  }, [mirrors, setMessages]);

  return { sendMessage, stop, reset };
}

/**
 * 会话基建 hook（explore 会话 / agent 调试 / 未来 agent 会话页面统一消费）。
 * 入参变更（source 二元组）触发重放装载；发送 / 停止 / 重置见模块文档。
 */
export function useAgentChat(params: UseAgentChatParams): UseAgentChatState {
  const mirrors = useSessionMirrors(params.source, params.sourceRef);

  const transport = useMemo(
    () => new TauriAgentTransport({ onEvent: mirrors.handleEvent, onRecord: mirrors.handleRecord }),
    [mirrors.handleEvent, mirrors.handleRecord],
  );

  // 收口归一经 ref 延接 setMessages（onFinish 闭包在 useChat 初始化前构造）
  const setMessagesRef = useRef<(next: AgentUIMessage[]) => void>(() => {});
  const chat = useChat<AgentUIMessage>({
    transport,
    onFinish: () => {
      // 流结束（含失败）按镜像重建，实时路与重放路同形状
      setMessagesRef.current(buildMessages(mirrors.eventsRef.current, mirrors.turnsRef.current));
    },
  });
  const { messages, status, error, setMessages } = chat;
  setMessagesRef.current = setMessages;

  // 镜像经 ref 供效果读取（对象每渲染重建，不能进效果依赖）
  const mirrorsRef = useRef(mirrors);
  mirrorsRef.current = mirrors;

  const replay = useSessionReplay(params, mirrorsRef, setMessages);
  const actions = useSessionActions(chat, mirrorsRef.current, params, setMessages);

  return {
    messages,
    events: mirrors.events,
    session: mirrors.session,
    chain: mirrors.turns,
    loading: replay.loading,
    running: status === 'submitted' || status === 'streaming',
    error: replay.error ?? (error !== undefined ? error.message : null),
    ...actions,
  };
}
