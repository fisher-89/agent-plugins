import { useChat } from '@ai-sdk/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { foldSessionMessages, type AgentUIMessage } from '../lib/agent-adapter';
import { TauriAgentTransport } from '../lib/agent-transport';
import { ipc } from '../lib/ipc';
import {
  type AgentEvent,
  type AgentPermissionMode,
  type SessionSummary,
  type TurnSummary,
} from '../types/generated/bindings';

/** 流式 messages 通知节流窗（ms）：delta 合并到约 10fps。useChat 只节流 React
 * 通知，status/error 不受影响，终态（ready/error）会强制同步最新 messages，
 * 因此不丢末段内容。 */
const STREAM_RENDER_THROTTLE_MS = 100;

/** 会话基建参数（root 寻址所属 workspace 库；sessionId=null 即不重放装载，
 * 会话 id 由发起后 early-resolve 轮行回写） */
export interface UseAgentSessionParams {
  root: string | null;
  sessionId: string | null;
  /** 来源归属透传（debug / explore / ...） */
  source: string;
  /** 来源内定位透传（归属记录 / 清单筛选；未提供即 null——既有消费面零漂移） */
  sourceRef?: string | null;
  /** 会话执行根透传（未提供即 null → 后端回落 `root`） */
  cwd?: string | null;
  /** 会话状态重评估信号：变化即重评估（重放装载 + 是否挂流） */
  refreshKey?: number;
}

/** 单次发送入参 */
export interface AgentSessionSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode | null;
  agent?: number | null;
}

/** 会话基建状态面（镜像 + 行为；类型经本面结构化流转，视图层不 import ai） */
export interface UseAgentSessionState {
  messages: AgentUIMessage[];
  events: AgentEvent[];
  /** 当前会话镜像（重放装载为最新会话；运行中为 early-resolve 回写的会话） */
  session: SessionSummary | null;
  /** 轮统计行列表（发起顺序；运行中含 running 行） */
  chain: TurnSummary[];
  loading: boolean;
  /** 库态运行标记（轮行含 running）：MUST NOT 以发送方视角 `chat.status` 替代 */
  running: boolean;
  error: string | null;
  sendMessage: (input: AgentSessionSendInput) => void;
  stop: () => void;
  reset: () => void;
  /** ask 应答（通用侧）：`agent_answer` 薄包装；`root` / `sessionId` 为 null 时
   * 不 invoke、`Err` 进既有 error 面 */
  answer: (text: string) => void;
  /** 补充说明注入（运行中会话的 steer）：`agent_steer` 薄包装；`root` /
   * `sessionId` 为 null 时不 invoke、`Err` 进既有 error 面 */
  steer: (text: string) => void;
  /** 已提交、等 ack 回流的 pending 态（提交置 true，ack 成功或 Err 落地置 false） */
  steering: boolean;
}

/** `useChat` 返回值（chat 状态层句柄；本 hook 内聚，视图层不 import ai） */
type AgentChat = ReturnType<typeof useChat<AgentUIMessage>>;

/** 在挂流状态（`submitted` / `streaming` 即「有在挂流」） */
function isStreaming(status: AgentChat['status']): boolean {
  return status === 'submitted' || status === 'streaming';
}

/** `useChat` 的消息状态写入面：整表替换（重放 / 收口折叠）或函数式 updater
 * （提问归一 / 追加——流式期间 ref 快照会滞后，MUST 用 updater） */
type SetAgentMessages = (
  updater: AgentUIMessage[] | ((messages: AgentUIMessage[]) => AgentUIMessage[]),
) => void;

/**
 * 提问气泡 id：`turn-<turnId>-user`（与 assistant `turn-<turnId>` 同族）；
 * `turnId` 缺席（旧转录 / 无轮归属构造面）兜 `evt-<seq>`。
 */
function questionIdOf(event: AgentEvent): string {
  return event.turnId != null ? `turn-${event.turnId}-user` : `evt-${event.seq}`;
}

/** 提问部件的直映射（提问块恒为 text；适配层计划层私有件不为此开导出） */
function questionParts(
  blocks: Extract<AgentEvent, { kind: 'message' }>['blocks'],
): AgentUIMessage['parts'] {
  const parts: AgentUIMessage['parts'] = [];
  for (const block of blocks) {
    if (block.kind === 'text') parts.push({ type: 'text', text: block.text, state: 'done' });
  }
  return parts;
}

/**
 * user 消息（提问 / 补充说明）的会话状态侧产生。判据 = **该轮提问气泡是否已在
 * 场**（`turn-<turnId>-user` 或旧转录 `evt-<seq>`）：首条 user 事件按提问归一
 * （①发起流已在发送时刻本地推入副本→把该条原地换 id，消息数不增；②订阅流无
 * 本地副本→追加在该轮 assistant 消息**之前**）；其余 user 事件是该轮补充说明，
 * 以自身 `evt-<seq>` 追加、MUST NOT 被「同轮已有提问」判据吞掉。
 * 两路一律经函数式 updater 单次写入（非函数式会覆盖掉在飞的 assistant 消息）。
 */
function applyUserMessage(
  setMessagesRef: React.RefObject<SetAgentMessages | undefined>,
  pendingQuestionIdRef: React.RefObject<string | null>,
  event: Extract<AgentEvent, { kind: 'message' }>,
): void {
  const setMessages = setMessagesRef.current;
  if (setMessages === undefined) return;
  const questionId = questionIdOf(event);
  const metadata = { seq: event.seq, parentToolUseId: event.parentToolUseId };
  const pendingId = pendingQuestionIdRef.current;
  pendingQuestionIdRef.current = null;
  setMessages((messages) => {
    // 判据读的是「提问气泡在场」而非「本地副本在场」：旁观 / 重挂路径没有本地
    // 副本，其首条 user 事件仍须按提问处理
    const isQuestion =
      event.turnId == null || !messages.some((message) => message.id === questionId);
    const id = isQuestion ? questionId : `evt-${event.seq}`;
    // 重订 / 收口竞态下同一事件重投：id 幂等，不新增重复气泡
    if (messages.some((message) => message.id === id)) return messages;
    if (isQuestion && pendingId !== null && messages.some((message) => message.id === pendingId)) {
      return messages.map((message) =>
        message.id === pendingId ? { ...message, id, metadata } : message,
      );
    }
    const message: AgentUIMessage = {
      id,
      role: 'user',
      metadata,
      parts: questionParts(event.blocks),
    };
    // 锚点与提问同一条表达式：提问先落位，补充说明自然紧随其后
    const anchor =
      event.turnId == null ? -1 : messages.findIndex((item) => item.id === `turn-${event.turnId}`);
    if (anchor < 0) return [...messages, message];
    return [...messages.slice(0, anchor), message, ...messages.slice(anchor)];
  });
}

/** 会话镜像与轮行的会话域同步（row 不变、stats/turns 现算） */
function withTurns(session: SessionSummary, turns: TurnSummary[]): SessionSummary {
  return { ...session, stats: { ...session.stats, turnCount: turns.length }, turns };
}

/** 运行中轮行的会话镜像（early-resolve 回写；字段以轮行为准的运行时投影） */
function sessionFromTurn(record: TurnSummary, source: string): SessionSummary {
  return {
    row: {
      id: record.sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source, sourceRef: null },
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
  /** 已观测事件的最大 seq（含 delta；订阅 `since_seq` 口径，重订不重投） */
  seqCursorRef: React.RefObject<number>;
  /** 收到的轮终态条数（订阅「无进展」判别的另一半） */
  recordCountRef: React.RefObject<number>;
  /** 发起流本地推入的提问副本临时 id（提问事件到达时归一并清空） */
  pendingQuestionIdRef: React.RefObject<string | null>;
  handleEvent: (event: AgentEvent) => void;
  handleRecord: (record: TurnSummary) => void;
  setSession: (session: SessionSummary | null) => void;
  setTurns: (turns: TurnSummary[]) => void;
  setEvents: (events: AgentEvent[]) => void;
  clear: () => void;
}

/** 轮行 upsert（按 turnId 替换 / 追加）并同步会话镜像的轮数面 */
function upsertTurnWrites(
  sessionRef: React.RefObject<SessionSummary | null>,
  turnsRef: React.RefObject<TurnSummary[]>,
  setSession: (session: SessionSummary | null) => void,
  setTurns: (turns: TurnSummary[]) => void,
): (record: TurnSummary) => void {
  return (record: TurnSummary) => {
    const next = [...turnsRef.current];
    const index = next.findIndex((turn) => turn.turnId === record.turnId);
    if (index >= 0) next[index] = record;
    else next.push(record);
    turnsRef.current = next;
    setTurns(next);
    const current = sessionRef.current;
    if (current === null) return;
    const synced = withTurns(current, next);
    sessionRef.current = synced;
    setSession(synced);
  };
}

/** 轮行写入回调装配（upsert / 终态替换 / 清空）：纯回调，不持有状态 */
function chainWrites(
  sessionRef: React.RefObject<SessionSummary | null>,
  turnsRef: React.RefObject<TurnSummary[]>,
  eventsRef: React.RefObject<AgentEvent[]>,
  currentSessionIdRef: React.RefObject<string | null>,
  seqCursorRef: React.RefObject<number>,
  recordCountRef: React.RefObject<number>,
  setSession: (session: SessionSummary | null) => void,
  setTurns: (turns: TurnSummary[]) => void,
  setEvents: (events: AgentEvent[]) => void,
  source: string,
): {
  upsertTurn: (record: TurnSummary) => void;
  handleRecord: (record: TurnSummary) => void;
  clear: () => void;
} {
  const upsertTurn = upsertTurnWrites(sessionRef, turnsRef, setSession, setTurns);
  // transport 观测点：early-resolve running 轮行（回写当前会话 id）/ 终态
  // 轮行按轮 id 替换
  const handleRecord = (record: TurnSummary) => {
    recordCountRef.current += 1;
    currentSessionIdRef.current = record.sessionId;
    if (sessionRef.current?.row.id !== record.sessionId) {
      const mirror = sessionFromTurn(record, source);
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
    seqCursorRef.current = 0;
    recordCountRef.current = 0;
    setSession(null);
    setTurns([]);
    setEvents([]);
  };
  return { upsertTurn, handleRecord, clear };
}

/** transport 观测点的 ref 面（写游标 / 追加镜像 / 提问归一读同一批 ref） */
interface EventTapRefs {
  /** 已观测事件的最大 seq（含 delta——订阅 `since_seq` 口径） */
  seqCursorRef: React.RefObject<number>;
  /** 密封事件镜像（append 保序；delta 不入） */
  eventsRef: React.RefObject<AgentEvent[]>;
  /** 发起流本地提问副本的临时 id */
  pendingQuestionIdRef: React.RefObject<string | null>;
  setEvents: (events: AgentEvent[]) => void;
  setMessagesRef: React.RefObject<SetAgentMessages | undefined>;
}

/**
 * transport 观测分派（镜像 ref 真相面 + 渲染态）：delta 只推高 seq 游标不入
 * 镜像；密封事件 append 保序，并在 user 事件处驱动会话状态侧的提问 / 补充说明
 * 气泡（transport 对该事件零 chunk）。
 */
function acceptEvent(tap: EventTapRefs, event: AgentEvent): void {
  if (event.seq > tap.seqCursorRef.current) tap.seqCursorRef.current = event.seq;
  if (event.kind === 'messageDelta') return; // delta 仅实时流可见
  const next = [...tap.eventsRef.current, event];
  tap.eventsRef.current = next;
  tap.setEvents(next);
  if (event.kind === 'message' && event.role === 'user') {
    applyUserMessage(tap.setMessagesRef, tap.pendingQuestionIdRef, event);
  }
}

/** 镜像与 transport 观测回调：running 轮行回写会话、终态按轮 id 替换、密封
 * 事件入镜像（delta 只上 chunk 流不入镜像，但同样推高 seq 游标） */
function useSessionMirrors(
  source: string,
  setMessagesRef: React.RefObject<SetAgentMessages | undefined>,
): SessionMirrors {
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [turns, setTurns] = useState<TurnSummary[]>([]);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const refs = {
    sessionRef: useRef<SessionSummary | null>(null),
    turnsRef: useRef<TurnSummary[]>([]),
    eventsRef: useRef<AgentEvent[]>([]),
    currentSessionIdRef: useRef<string | null>(null),
    seqCursorRef: useRef(0),
    recordCountRef: useRef(0),
    pendingQuestionIdRef: useRef<string | null>(null),
  };
  const { handleRecord, clear } = chainWrites(
    refs.sessionRef,
    refs.turnsRef,
    refs.eventsRef,
    refs.currentSessionIdRef,
    refs.seqCursorRef,
    refs.recordCountRef,
    setSession,
    setTurns,
    setEvents,
    source,
  );
  // ref 成员与 setter 全跨渲染稳定，故回调身份稳定（transport 不必重建）
  const handleEvent = useCallback(
    (event: AgentEvent) => acceptEvent({ ...refs, setEvents, setMessagesRef }, event),
    [setMessagesRef],
  );

  return {
    session,
    turns,
    events,
    ...refs,
    handleEvent,
    handleRecord,
    setSession,
    setTurns,
    setEvents,
    clear,
  };
}

/** 重放取数：sessionId 直查 sessionDetail → 密封转录；sessionId=null → 空 */
async function loadReplay(
  root: string,
  sessionId: string,
): Promise<{ latest: SessionSummary | null; events: AgentEvent[] }> {
  const detail = await ipc.sessionDetail(root, sessionId);
  if (detail === null) return { latest: null, events: [] };
  const events = await ipc.agentSessionTranscript(root, sessionId);
  return { latest: detail, events };
}

/** 重放结果回写：镜像真相（ref）与渲染态、消息重建一并落位（游标同步推到
 * 已装载事件的最大 seq——续订不重投） */
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
  mirrors.seqCursorRef.current = events.reduce((max, event) => Math.max(max, event.seq), 0);
  mirrors.setSession(latest);
  mirrors.setTurns(latest?.turns ?? []);
  mirrors.setEvents(events);
  setMessages(foldSessionMessages(events, latest?.turns ?? []));
}

/** 重放装载产物：在途标记（同步可见，订阅调度据此让位）+ 完成拍（重放落地
 * 即自增，驱动订阅调度重评估）+ 装载态 / 错误面 */
interface SessionReplay {
  loading: boolean;
  error: string | null;
  /** 重放在途（ref，效果内同步置位——订阅调度读它不受渲染快照滞后影响） */
  pendingRef: React.RefObject<boolean>;
  /** 重放完成拍（成功 / 失败均自增；订阅调度据其以新游标重评估） */
  replayTick: number;
}

/**
 * 单轮重放取数：成功即回写镜像与消息（同一折叠入口重建），失败回落本地事件
 * 镜像折叠（收口不阻塞、不抛错）；两种结局都回调 `onSettled`（完成拍驱动
 * 订阅重评估）。
 */
function runReplayLoad(
  root: string,
  targetId: string,
  mirrors: SessionMirrors,
  setMessages: (messages: AgentUIMessage[]) => void,
  isCancelled: () => boolean,
  onSettled: (error: string | null) => void,
): void {
  loadReplay(root, targetId)
    .then(({ latest, events }) => {
      if (isCancelled()) return;
      applyReplay(mirrors, setMessages, latest, events);
      onSettled(null);
    })
    .catch((cause: unknown) => {
      if (isCancelled()) return;
      setMessages(foldSessionMessages(mirrors.eventsRef.current, mirrors.turnsRef.current));
      onSettled(String(cause));
    });
}

/**
 * 重放装载与收口收敛单点：
 * - 效果 A（空参复位）：`root` / `sessionId` 缺席即清镜像与消息，零取数零订阅；
 * - 效果 B（`root` / `sessionId` / `refreshKey` / 在挂流状态）：重放装载 + 收口
 *   收敛——在挂流期间不中途重放；流结束后按库内转录重建（终态以库为唯一权威），
 *   重放失败回落本地事件镜像折叠（不抛错、不阻塞）。
 */
function useSessionReplay(
  params: UseAgentSessionParams,
  mirrorsRef: React.RefObject<SessionMirrors>,
  status: AgentChat['status'],
  setMessages: (messages: AgentUIMessage[]) => void,
): SessionReplay {
  const { root, sessionId, refreshKey } = params;
  const [loading, setLoading] = useState(false);
  const [replayError, setReplayError] = useState<string | null>(null);
  const [replayTick, setReplayTick] = useState(0);
  const pendingRef = useRef(false);

  useEffect(() => {
    if (root !== null && sessionId !== null) return;
    mirrorsRef.current.clear();
    pendingRef.current = false;
    setMessages([]);
    setLoading(false);
    setReplayError(null);
  }, [root, sessionId, mirrorsRef, setMessages]);

  useEffect(() => {
    const mirrors = mirrorsRef.current;
    const targetId = sessionId ?? mirrors.currentSessionIdRef.current;
    if (root === null || targetId === null) return;
    // 在挂流不中途重放（一条判据同时挡住双流与中途重放）
    if (isStreaming(status)) return;
    let cancelled = false;
    // 先全量重放再续订：在途标记本拍同步置位（订阅调度据此让位）
    pendingRef.current = true;
    setLoading(true);
    setReplayError(null);
    runReplayLoad(
      root,
      targetId,
      mirrors,
      setMessages,
      () => cancelled,
      (settledError) => {
        if (cancelled) return;
        pendingRef.current = false;
        setReplayError(settledError);
        setLoading(false);
        setReplayTick((tick) => tick + 1);
      },
    );
    return () => {
      cancelled = true;
      pendingRef.current = false;
    };
  }, [root, sessionId, refreshKey, status, mirrorsRef, setMessages]);

  return { loading, error: replayError, pendingRef, replayTick };
}

/** 订阅调度的无进展闸键（D15）：目标会话 + 重评估键 + 订阅前游标 */
function blockedKey(targetId: string, refreshKey: number | undefined, cursor: number): string {
  return `${targetId}#${refreshKey ?? 0}#${cursor}`;
}

/**
 * 订阅调度（实时面第二起点）：会话库态运行中且无在挂流时以 `sinceSeq` = 已
 * 观测最大 seq 订阅；单流在挂不变式由「在挂流即不调度」一条判据守住。
 * 订阅结束而游标未前进且未收到 `Record`（库态 running 但无实时流：应用重启
 * 遗留轮行 / 收口与重放竞态）→ 以 `(目标会话, refreshKey, 订阅前游标)` 记阻断，
 * 直到 `sessionId` / `refreshKey` 变化才重试（防 invoke 自旋，不引入轮询）。
 */
function useSessionStreamSubscription(
  params: UseAgentSessionParams,
  mirrorsRef: React.RefObject<SessionMirrors>,
  resumeStream: AgentChat['resumeStream'],
  running: boolean,
  replay: SessionReplay,
  status: AgentChat['status'],
): void {
  const { root, sessionId, refreshKey } = params;
  const { pendingRef, replayTick } = replay;
  const blockedRef = useRef<string | null>(null);
  useEffect(() => {
    const mirrors = mirrorsRef.current;
    const targetId = sessionId ?? mirrors.currentSessionIdRef.current;
    if (root === null || targetId === null || !running) return;
    // 单流在挂：发起流 / 既有订阅在挂时不另建订阅（防同轮增量重复注入）
    if (isStreaming(status)) return;
    // 先全量重放再续订：重放在途即让位（完成拍 replayTick 触发重评估）
    if (pendingRef.current) return;
    const before = mirrors.seqCursorRef.current;
    const key = blockedKey(targetId, refreshKey, before);
    if (blockedRef.current === key) return;
    const recordsBefore = mirrors.recordCountRef.current;
    void resumeStream({ body: { root, sessionId: targetId, sinceSeq: before } })
      .catch(() => undefined)
      .finally(() => {
        const progressed =
          mirrors.seqCursorRef.current > before || mirrors.recordCountRef.current > recordsBefore;
        blockedRef.current = progressed ? null : key;
      });
  }, [
    root,
    sessionId,
    running,
    status,
    refreshKey,
    pendingRef,
    replayTick,
    mirrorsRef,
    resumeStream,
  ]);
}

/** 发起流 body 的会话参数面（来源侧字段原样透传；transport 为无状态转换器） */
interface StartBodyParams {
  root: string;
  source: string;
  sessionId: string | null;
  sourceRef: string | null;
  cwd: string | null;
  /** 镜像回写的当前会话（early-resolve）；未确立时回落来源侧会话引用 */
  currentSessionId: string | null;
}

/** 发起 invoke 的 body 组装（八个字段唯一构造点） */
function startBody(
  session: StartBodyParams,
  input: AgentSessionSendInput,
): Record<string, unknown> {
  return {
    root: session.root,
    prompt: input.prompt,
    permissionMode: input.permissionMode,
    agent: input.agent ?? null,
    // 当前会话 Continue（early-resolve 轮行回写）；镜像尚未确立（重放未落地 /
    // 失败）时回落来源侧的会话引用——来源已有会话就绝不新铸
    sessionId: session.currentSessionId ?? session.sessionId,
    source: session.source,
    // 来源侧归属与执行根原样透传（未提供即 null：执行根回落 root、归属
    // sourceRef 为 null——调试页 / explore 链语义零漂移）
    sourceRef: session.sourceRef,
    cwd: session.cwd,
  };
}

/** 会话行为（发送组装 / 停止 / 重置）；会话参数在此收口、body 穿透 transport */
function useSessionActions(
  chat: AgentChat,
  statusRef: React.RefObject<AgentChat['status']>,
  mirrors: SessionMirrors,
  params: UseAgentSessionParams,
  setMessages: (messages: AgentUIMessage[]) => void,
): {
  sendMessage: (input: AgentSessionSendInput) => void;
  stop: () => void;
  reset: () => void;
} {
  const { source, sourceRef, cwd, sessionId, root } = params;
  const send = chat.sendMessage;
  // 本地提问副本的临时 id 计数（归一后即弃，MUST NOT 出现在任何持久面 / IPC 载荷）
  const pendingSeqRef = useRef(0);
  const session: Omit<StartBodyParams, 'currentSessionId'> = {
    root: root ?? '',
    source,
    sessionId,
    sourceRef: sourceRef ?? null,
    cwd: cwd ?? null,
  };

  const sendMessage = useCallback(
    (input: AgentSessionSendInput) => {
      if (root === null) return;
      // 在挂流重复发送忽略（status 实例属性直读，同步无竞态）
      if (isStreaming(statusRef.current)) return;
      pendingSeqRef.current += 1;
      const pendingId = `pending-user-${pendingSeqRef.current}`;
      // 提问由会话状态侧本地推入：`sendMessage` 在 `await makeRequest` 之前同步
      // pushMessage，故本地副本结构上先于任何 chunk（「思考跑到提问之前」因而
      // 不可能）；该轮提问事件到达时归一到 `turn-<turnId>-user`
      mirrors.pendingQuestionIdRef.current = pendingId;
      void send(
        { id: pendingId, parts: [{ type: 'text', text: input.prompt }] },
        {
          body: startBody(
            { ...session, currentSessionId: mirrors.currentSessionIdRef.current },
            input,
          ),
        },
      );
    },
    [send, statusRef, mirrors.currentSessionIdRef, mirrors.pendingQuestionIdRef, session, root],
  );

  const stop = useCallback(() => {
    const sessionId = mirrors.currentSessionIdRef.current;
    // 不调 chat.stop() 截断前端流：终态轮行由后端经在挂流以 `Record` 信封收尾
    // （前端挂为该轮 `metadata.record`）；停止寻址携 root（会话 id 即寻址键）
    if (sessionId !== null && root !== null) void ipc.agentStop(root, sessionId);
  }, [mirrors.currentSessionIdRef, root]);

  const reset = useCallback(() => {
    mirrors.clear();
    setMessages([]);
  }, [mirrors, setMessages]);

  return { sendMessage, stop, reset };
}

/**
 * ask 应答动作面：`agent_answer` 薄包装（`root` / `sessionId` 为 null 时零
 * invoke）；`Err` 收进 `actionError`（design DD11 的第三源，与重放 / 流错误
 * 以 `??` 链保序合并——一次应答失败不遮蔽更权威的重放错误）。
 */
function useAskAnswer(
  mirrors: SessionMirrors,
  sessionId: string | null | undefined,
  root: string | null | undefined,
): { answer: (text: string) => void; actionError: string | null } {
  const [actionError, setActionError] = useState<string | null>(null);
  const answer = useCallback(
    (text: string) => {
      const target = mirrors.currentSessionIdRef.current ?? sessionId ?? null;
      if (root === null || root === undefined || target === null) return;
      setActionError(null);
      void ipc
        .agentAnswer(root, target, text)
        .catch((cause: unknown) => setActionError(String(cause)));
    },
    [mirrors.currentSessionIdRef, sessionId, root],
  );
  return { answer, actionError };
}

/**
 * 补充说明动作面：`agent_steer` 薄包装（`root` / `sessionId` 为 null 时零
 * invoke）；`steering` 是「已提交、等 ack 回流」的 pending 态（提交置位、成功
 * 与 `Err` 两收尾都复位）；`Err` 收进 `actionError`（与重放 / 流 / 应答错误以
 * `??` 链保序合并）。前端不预判可用性——轮是否在跑由后端 `Err` 说话。
 */
function useSteer(
  mirrors: SessionMirrors,
  sessionId: string | null | undefined,
  root: string | null | undefined,
): { steer: (text: string) => void; steering: boolean; actionError: string | null } {
  const [actionError, setActionError] = useState<string | null>(null);
  const [steering, setSteering] = useState(false);
  const steer = useCallback(
    (text: string) => {
      const target = mirrors.currentSessionIdRef.current ?? sessionId ?? null;
      if (root === null || root === undefined || target === null) return;
      setActionError(null);
      setSteering(true);
      void ipc
        .agentSteer(root, target, text)
        .catch((cause: unknown) => setActionError(String(cause)))
        .finally(() => setSteering(false));
    },
    [mirrors.currentSessionIdRef, sessionId, root],
  );
  return { steer, steering, actionError };
}

/**
 * 错误面保序合并（重放 > 流 > 动作）：一次动作失败不遮蔽更权威的重放错误；
 * 两个动作源（ask / steer）同层，先后只保证确定性。
 */
function errorChain(
  replayError: string | null,
  streamError: string | undefined,
  askError: string | null,
  steerError: string | null,
): string | null {
  return replayError ?? streamError ?? askError ?? steerError;
}

/**
 * 统一会话基建 hook（explore 会话 / agent 调试 / change 抽屉 / 历史重放统一
 * 消费，收编 `use-agent-chat`）：重放装载（密封转录 → 同一折叠入口）+ 状态驱动
 * 挂流（库态 running 且无在挂流才订阅）+ 单流在挂 + 收口以重放收敛（失败回落
 * 镜像折叠）+ 断流重订（游标 / 无进展闸）+ 发送 / 停止 / 重置。
 * `running` 为库态派生（轮行含 running），MUST NOT 以 `chat.status` 替代。
 */
export function useAgentSession(params: UseAgentSessionParams): UseAgentSessionState {
  // 消息状态写入面经 ref 延接：`onFinish` 闭包在 `useChat` 初始化前构造，提问
  // 归一（镜像侧）与收口折叠（本 hook）写的是同一份消息状态
  const setMessagesRef = useRef<SetAgentMessages | undefined>(undefined);
  const mirrors = useSessionMirrors(params.source, setMessagesRef);

  const transport = useMemo(
    () =>
      new TauriAgentTransport({
        onEvent: mirrors.handleEvent,
        onRecord: mirrors.handleRecord,
        // 订阅起点播种源：重放装载后的密封事件镜像（resume 流从空态起步，
        // 不播种则运行轮的既有部件会被增量独占覆盖）
        observedEvents: () => mirrors.eventsRef.current,
      }),
    [mirrors.handleEvent, mirrors.handleRecord, mirrors.eventsRef],
  );

  const chat = useChat<AgentUIMessage>({
    transport,
    throttle: STREAM_RENDER_THROTTLE_MS,
    onFinish: () => {
      // 流结束（含失败 / 断流）即时按镜像折叠归一；收口重放随后覆盖（库为准）
      setMessagesRef.current?.(
        foldSessionMessages(mirrors.eventsRef.current, mirrors.turnsRef.current),
      );
    },
  });
  const { messages, status, error, setMessages, resumeStream } = chat;
  setMessagesRef.current = setMessages;

  // 镜像与在挂流状态经 ref 供效果 / 动作读取（对象每渲染重建，不能进效果依赖）
  const mirrorsRef = useRef(mirrors);
  mirrorsRef.current = mirrors;
  const statusRef = useRef(status);
  statusRef.current = status;

  const replay = useSessionReplay(params, mirrorsRef, status, setMessages);
  const actions = useSessionActions(chat, statusRef, mirrors, params, setMessages);
  const ask = useAskAnswer(mirrors, params.sessionId, params.root);
  const steerAction = useSteer(mirrors, params.sessionId, params.root);
  // 库态派生：轮行含 running 即运行中（与查询面判定同源）
  const running = mirrors.turns.some((turn) => turn.status === 'running');
  useSessionStreamSubscription(params, mirrorsRef, resumeStream, running, replay, status);

  return {
    messages,
    events: mirrors.events,
    session: mirrors.session,
    chain: mirrors.turns,
    loading: replay.loading,
    running,
    error: errorChain(replay.error, error?.message, ask.actionError, steerAction.actionError),
    ...actions,
    answer: ask.answer,
    steer: steerAction.steer,
    steering: steerAction.steering,
  };
}
