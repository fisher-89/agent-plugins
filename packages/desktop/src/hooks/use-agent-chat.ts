/**
 * headless 会话基建 hook：ai-sdk v7 `useChat` + `TauriAgentTransport` 承载
 * agent 会话的 UIMessage 状态（spec：desktop-agent-chat-infra）。
 *
 * - 重放装载：`agent_run_chain` + 逐 run `agent_run_events` → 适配层重建 →
 *   setMessages；逐 run 交错合成 record 部件（终态 run 才有）。重放重建与
 *   实时收口归一后同一状态形状（重开恢复 = 重放）。
 * - 发送组装：链尾取最后一条**非 running** 记录（stop 后 record 未达的竞态
 *   防御），`resumeSessionId` / `parentRunId` 与来源三元组经 body 穿透；
 *   会话文本组装（如 explore stance 拼接）留在来源侧。运行中重复发送忽略。
 * - 停止：`stop` → `invoke("agent_stop")`，不调 chat.stop() 截断前端流
 *   （终态 record 部件与 finish 由后端闭流推入）。
 * - 收口归一：流结束时（含失败）按 events / chain 镜像重建一次消息状态，
 *   把实时路的角色/配对呈现归一到重放形状（两路同构）。
 *
 * transport 的 tee 回调经稳定引用注入（观测点不承载状态）；链状态归本 hook。
 */

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
  type AgentRunRecord,
} from '../types/generated/bindings';

/** 会话来源参数（source 二元组 + cwd；sourceRef=null 即不重放装载） */
export interface UseAgentChatParams {
  source: string;
  sourceRef: string | null;
  root: string | null;
}

/** 单次发送入参（文本已由来源侧组装完毕） */
interface AgentChatSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
}

/** 会话基建状态面（镜像 + 行为；类型经本面结构化流转，视图层不 import ai） */
export interface UseAgentChatState {
  messages: AgentUIMessage[];
  events: AgentEvent[];
  chain: AgentRunRecord[];
  loading: boolean;
  running: boolean;
  error: string | null;
  currentRunId: number | null;
  sendMessage: (input: AgentChatSendInput) => void;
  stop: () => void;
  reset: () => void;
}

/** 链尾取最后一条非 running 记录（续话参数来源；running 过滤防御竞态） */
function chainTail(chain: AgentRunRecord[]): AgentRunRecord | null {
  for (let index = chain.length - 1; index >= 0; index -= 1) {
    if (chain[index].status !== 'running') return chain[index];
  }
  return null;
}

/** 镜像扁平化：链序逐 run 拼接事件（实时与重放共用同一口径） */
function flatEvents(chain: AgentRunRecord[], byRun: Map<number, AgentEvent[]>): AgentEvent[] {
  return chain.flatMap((run) => byRun.get(run.id) ?? []);
}

/** 重放/归一重建：逐 run 事件折叠 + 终态 record 部件交错（running 无 record） */
function buildMessages(
  chain: AgentRunRecord[],
  byRun: Map<number, AgentEvent[]>,
): AgentUIMessage[] {
  return chain.flatMap((run) => {
    const replayed = eventsToUIMessages(byRun.get(run.id) ?? []);
    return run.status === 'running' ? replayed : [...replayed, runRecordToUIMessage(run)];
  });
}

/** 链还原 + 逐 run 事件重放（store 链查询收口单点，hook 不拼链；root 寻址
 * 所属 workspace 库——run id 与 explore 记录 id 均为库域内） */
async function loadChain(
  root: string,
  source: string,
  sourceRef: string,
): Promise<{ runs: AgentRunRecord[]; byRun: Map<number, AgentEvent[]> }> {
  const runs = await commands.agentRunChain(root, source, sourceRef);
  const replays = await Promise.all(
    runs.map(async (run) => [run.id, await commands.agentRunEvents(root, run.id)] as const),
  );
  return { runs, byRun: new Map(replays) };
}

/** 镜像内部态（ref 真相 + state 渲染双轨，transport 回调与发送组装读 ref） */
interface SessionMirrors {
  chain: AgentRunRecord[];
  events: AgentEvent[];
  currentRunId: number | null;
  chainRef: React.RefObject<AgentRunRecord[]>;
  eventsByRunRef: React.RefObject<Map<number, AgentEvent[]>>;
  currentRunIdRef: React.RefObject<number | null>;
  handleEvent: (event: AgentEvent) => void;
  handleRecord: (record: AgentRunRecord) => void;
  setChain: (runs: AgentRunRecord[]) => void;
  setEvents: (events: AgentEvent[]) => void;
  setCurrentRunId: (runId: number | null) => void;
  clear: () => void;
}

/** 链写入回调装配（upsert / 终态替换 / 清空）：纯回调，不持有状态 */
function chainWrites(
  chainRef: React.RefObject<AgentRunRecord[]>,
  eventsByRunRef: React.RefObject<Map<number, AgentEvent[]>>,
  currentRunIdRef: React.RefObject<number | null>,
  setChain: (runs: AgentRunRecord[]) => void,
  setEvents: (events: AgentEvent[]) => void,
  setCurrentRunId: (runId: number | null) => void,
): {
  upsertRun: (record: AgentRunRecord) => void;
  handleRecord: (record: AgentRunRecord) => void;
  clear: () => void;
} {
  const upsertRun = (record: AgentRunRecord) => {
    const next = [...chainRef.current];
    const index = next.findIndex((run) => run.id === record.id);
    if (index >= 0) next[index] = record;
    else next.push(record);
    chainRef.current = next;
    setChain(next);
  };
  // transport 观测点：early-resolve running 记录 / 终态 record 按 id 替换
  const handleRecord = (record: AgentRunRecord) => {
    upsertRun(record);
    if (record.status === 'running') {
      currentRunIdRef.current = record.id;
      setCurrentRunId(record.id);
    }
  };
  const clear = () => {
    chainRef.current = [];
    eventsByRunRef.current = new Map();
    currentRunIdRef.current = null;
    setChain([]);
    setEvents([]);
    setCurrentRunId(null);
  };
  return { upsertRun, handleRecord, clear };
}

/** 镜像与 transport 观测回调：running 记录追加、终态按 id 替换、事件入桶 */
function useSessionMirrors(): SessionMirrors {
  const [chain, setChain] = useState<AgentRunRecord[]>([]);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [currentRunId, setCurrentRunId] = useState<number | null>(null);
  const chainRef = useRef<AgentRunRecord[]>([]);
  const eventsByRunRef = useRef<Map<number, AgentEvent[]>>(new Map());
  const currentRunIdRef = useRef<number | null>(null);
  const { handleRecord, clear } = chainWrites(
    chainRef,
    eventsByRunRef,
    currentRunIdRef,
    setChain,
    setEvents,
    setCurrentRunId,
  );

  // transport 观测点：实时事件入当前 run 桶并同步扁平镜像
  const handleEvent = useCallback((event: AgentEvent) => {
    const runId = currentRunIdRef.current;
    if (runId === null) return;
    const bucket = eventsByRunRef.current.get(runId) ?? [];
    eventsByRunRef.current.set(runId, [...bucket, event]);
    setEvents(flatEvents(chainRef.current, eventsByRunRef.current));
  }, []);

  return {
    chain,
    events,
    currentRunId,
    chainRef,
    eventsByRunRef,
    currentRunIdRef,
    handleEvent,
    handleRecord,
    setChain,
    setEvents,
    setCurrentRunId,
    clear,
  };
}

/** 重放装载：source 二元组变更触发；镜像与消息状态按查询结果重建 */
function useChainReplay(
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
    loadChain(root, source, sourceRef)
      .then(({ runs, byRun }) => {
        if (cancelled) return;
        mirrors.chainRef.current = runs;
        mirrors.eventsByRunRef.current = byRun;
        const tail = chainTail(runs);
        mirrors.currentRunIdRef.current = tail?.id ?? null;
        mirrors.setChain(runs);
        mirrors.setEvents(flatEvents(runs, byRun));
        mirrors.setCurrentRunId(tail?.id ?? null);
        setMessages(buildMessages(runs, byRun));
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

/** 会话行为（发送组装 / 停止 / 重置）；链参数在此收口、body 穿透 transport */
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
      const tail = chainTail(mirrors.chainRef.current);
      void chat.sendMessage(undefined, {
        body: {
          root,
          prompt: input.prompt,
          permissionMode: input.permissionMode,
          resumeSessionId: tail?.sessionId ?? null,
          parentRunId: tail?.id ?? null,
          source,
          sourceRef,
        },
      });
    },
    [chat, mirrors.chainRef, root, source, sourceRef],
  );

  const stop = useCallback(() => {
    const runId = mirrors.currentRunIdRef.current;
    // 不调 chat.stop() 截断前端流：终态 record 部件与 finish 由后端闭流推入；
    // 停止寻址携 root（run id 为 workspace 库域内自增，复合键消解跨库歧义）
    if (runId !== null && root !== null) void commands.agentStop(root, runId);
  }, [mirrors.currentRunIdRef, root]);

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
  const mirrors = useSessionMirrors();

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
      setMessagesRef.current(
        buildMessages(mirrors.chainRef.current, mirrors.eventsByRunRef.current),
      );
    },
  });
  const { messages, status, error, setMessages } = chat;
  setMessagesRef.current = setMessages;

  // 镜像经 ref 供效果读取（对象每渲染重建，不能进效果依赖）
  const mirrorsRef = useRef(mirrors);
  mirrorsRef.current = mirrors;

  const replay = useChainReplay(params, mirrorsRef, setMessages);
  const actions = useSessionActions(chat, mirrorsRef.current, params, setMessages);

  return {
    messages,
    events: mirrors.events,
    chain: mirrors.chain,
    loading: replay.loading,
    running: status === 'submitted' || status === 'streaming',
    error: replay.error ?? (error !== undefined ? error.message : null),
    currentRunId: mirrors.currentRunId,
    ...actions,
  };
}
