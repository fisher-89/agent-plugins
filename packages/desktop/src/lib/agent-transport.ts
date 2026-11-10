import { Channel } from '@tauri-apps/api/core';
import type { ChatTransport } from 'ai';

import {
  type AgentDelta,
  type AgentEvent,
  type AgentRunMessage,
  type TurnSummary,
} from '../types/generated/bindings';
import {
  deltaPartId,
  eventToChunk,
  type AgentUIMessage,
  type AgentUIMessageChunk,
  type DeltaPartAnchor,
} from './agent-adapter';
import { ipc } from './ipc';

/** `ipc.agentStart` 位置参数面（生成绑定派生，无手写镜像） */
type AgentStartArgs = Parameters<typeof ipc.agentStart>;

/** 会话参数 + 提示词（hook 发送时刻组装，body 原样穿透；形状派生自生成绑定） */
type AgentStartSessionParams = {
  root: AgentStartArgs[1];
  prompt: AgentStartArgs[2];
  permissionMode: AgentStartArgs[3];
  sessionId: AgentStartArgs[4];
  source: AgentStartArgs[5];
  /** 来源内定位（归属记录 / 清单筛选；MUST NOT 用于寻址） */
  sourceRef: AgentStartArgs[6];
  agent: AgentStartArgs[7];
  /** 会话执行根（尾部可选位；null → 后端回落 `root`） */
  cwd: AgentStartArgs[8];
};

/** transport 构造观测点：hook 注入（不承载状态，仅透传镜像） */
export interface TauriAgentTransportOptions {
  onEvent?: (event: AgentEvent) => void;
  onRecord?: (record: TurnSummary) => void;
  /** 已装载密封事件镜像（订阅起点播种源；省略即不播种——发起流出口不用） */
  observedEvents?: () => readonly AgentEvent[];
}

/** unknown → AgentPermissionMode（清单外值拒绝，transport 不改写） */
function readPermissionMode(value: unknown): AgentStartSessionParams['permissionMode'] {
  if (value === 'default' || value === 'acceptEdits' || value === 'bypassPermissions') {
    return value;
  }
  throw new Error(`非法 permissionMode: ${JSON.stringify(value)}`);
}

/** unknown → number | null（agent 可缺席——null / 缺席 → 后端解析默认 agent；
 * 清单外值拒绝，transport 不改写） */
function readAgentId(value: unknown): AgentStartSessionParams['agent'] {
  if (value === undefined || value === null) return null;
  if (typeof value === 'number') return value;
  throw new Error(`非法 agent: ${JSON.stringify(value)}`);
}

/** unknown → string | null（缺席与 null 同义——未提供即 null；其余形态拒绝） */
function readNullableString(value: unknown, field: string): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value === 'string') return value;
  throw new Error(`非法 ${field}: ${JSON.stringify(value)}`);
}

/** body 逐字段读取（运行时校验，transport 不增删改写会话参数） */
function readSessionParams(body: object | undefined): AgentStartSessionParams {
  const get = (key: string): unknown =>
    Object.entries(body ?? {}).find(([name]) => name === key)?.[1];
  const root = get('root');
  const prompt = get('prompt');
  const source = get('source');
  if (typeof root !== 'string' || typeof prompt !== 'string' || typeof source !== 'string') {
    throw new Error('缺少 agent_start 会话参数（root / prompt / source）');
  }
  return {
    root,
    prompt,
    source,
    permissionMode: readPermissionMode(get('permissionMode')),
    sessionId: readNullableString(get('sessionId'), 'sessionId'),
    sourceRef: readNullableString(get('sourceRef'), 'sourceRef'),
    agent: readAgentId(get('agent')),
    cwd: readNullableString(get('cwd'), 'cwd'),
  };
}

/** 增量事件累积部件种类（思考/回复可辨；`messageDelta` 以外不适用） */
function deltaPartKind(delta: AgentDelta): 'text' | 'thinking' {
  return delta.kind === 'text' ? 'text' : 'thinking';
}

/** part-start chunk（text / reasoning 按 kind 各一；id 为已算好的 delta part id） */
function partStartChunk(id: string, part: 'text' | 'thinking'): AgentUIMessageChunk {
  return part === 'text' ? { type: 'text-start', id } : { type: 'reasoning-start', id };
}

/** 订阅流 body 参数（会话订阅面入参；`chat.resumeStream({ body })` 透传） */
interface SessionStreamParams {
  root: string;
  sessionId: string;
  /** 已观测事件的最大 seq（订阅面只投严格更大者，重订不重投） */
  sinceSeq: number;
}

/** body 逐字段读取：任一字段非法 / 缺席即无订阅参数（返回 null，`reconnectToStream`
 * 以 null 上报「无运行轮可续」） */
function readStreamParams(body: object | undefined): SessionStreamParams | null {
  const get = (key: string): unknown =>
    Object.entries(body ?? {}).find(([name]) => name === key)?.[1];
  const root = get('root');
  const sessionId = get('sessionId');
  const sinceSeq = get('sinceSeq');
  if (typeof root !== 'string' || typeof sessionId !== 'string' || typeof sinceSeq !== 'number') {
    return null;
  }
  return { root, sessionId, sinceSeq };
}

/** 信封流的簿记单点产物（发起流与订阅流共用同一套 step 分段与终态收口） */
interface EnvelopeStream {
  stream: ReadableStream<AgentUIMessageChunk>;
  /** Channel 信封入口（事件分派 + `Record` 收口；未知信封忽略不断流） */
  accept: (message: AgentRunMessage) => void;
  /** 结束流：未收尾即关流（不投伪终态——ai-sdk 据 close 复位 status 并触发
   * `onFinish`） */
  end: () => void;
  /** 注错断流（invoke 失败等） */
  fail: (cause: unknown) => void;
  /** 订阅流起点播种登记（仅订阅出口用）：首批实时事件到达时若同属 `turnId`，
   * 先按其重建既有部件（见 [`reconnectToStream`]） */
  expectSeed: (turnId: number | null, events: readonly AgentEvent[]) => void;
}

/**
 * 实时路流内状态（两出口共用同一簿记单点）：
 * - `messageStarted` / `messageId` / `startSeq`：每轮**仅一次** `start`
 *   （`messageId = turn-<turnId>`，与折叠路同源）；
 * - `stepOpen` / `partAnchors`：step = 一次模型响应（`start-step` 在响应首个
 *   可渲染动作处发，密封到达时 `reset-step` 开下一段）；`partAnchors` 为当前
 *   step 的 delta part 锚（kind → 首个同类 delta 的 seq，新 step 开出时清空）；
 * - `currentToolCalls`：**本轮累积**的工具调用 id（跨 step 不清——工具结果就地
 *   补全既有部件，不开新 step）。
 */
interface StreamState {
  messageStarted: boolean;
  messageId: string | null;
  startSeq: number | null;
  stepOpen: boolean;
  partAnchors: Map<'text' | 'thinking', number>;
  currentToolCalls: Set<string>;
}

/** 轮内首个信封事件发一次 `start`（已建消息即返回空——`turn-<turnId>` 单点） */
function startChunks(
  state: StreamState,
  messageId: string,
  seq: number | null,
): AgentUIMessageChunk[] {
  state.messageStarted = true;
  state.messageId = messageId;
  state.startSeq = seq;
  return [{ type: 'start', messageId, messageMetadata: { seq, parentToolUseId: null } }];
}

/** delta part 锚解析（`eventToChunk` 第三参）：当前 step 内首个同类 delta 的 seq */
function anchorOf(state: StreamState): DeltaPartAnchor {
  return (kind, seq) => state.partAnchors.get(kind) ?? seq;
}

/** delta 事件的帧：新响应首 delta 前开 step，同类 delta 只在首个处发 part-start */
function deltaFrame(state: StreamState, event: Extract<AgentEvent, { kind: 'messageDelta' }>) {
  const chunks: AgentUIMessageChunk[] = [];
  const kind = deltaPartKind(event.delta);
  if (!state.stepOpen) {
    chunks.push({ type: 'start-step' });
    state.stepOpen = true;
    state.partAnchors.clear();
  }
  if (!state.partAnchors.has(kind)) {
    state.partAnchors.set(kind, event.seq);
    chunks.push(partStartChunk(deltaPartId(event.seq, kind), kind));
  }
  chunks.push(...eventToChunk(event, state.currentToolCalls, anchorOf(state)));
  return chunks;
}

/**
 * 密封 assistant 事件的帧：**该响应无 delta 时先开 step**（否则 `reset-step`
 * 会按「最后一个 `step-start` 之后」把上一段的密封块一并清掉），随后**恒发一次
 * `reset-step`**（只清本 step 的临时部件，前面 step 的密封块存活）再追加整块。
 */
function sealedFrame(
  state: StreamState,
  event: Extract<AgentEvent, { kind: 'message' }>,
): AgentUIMessageChunk[] {
  const chunks: AgentUIMessageChunk[] = [];
  if (!state.stepOpen) chunks.push({ type: 'start-step' });
  chunks.push({ type: 'reset-step' });
  state.stepOpen = false;
  state.partAnchors.clear();
  chunks.push(...eventToChunk(event, state.currentToolCalls, anchorOf(state)));
  for (const block of event.blocks) {
    if (block.kind === 'toolUse') state.currentToolCalls.add(block.id);
  }
  return chunks;
}

/** 信封分派产物：待投 chunk 组 + 该信封是否收口关流（未知信封两者皆空） */
interface EnvelopeOutcome {
  chunks: AgentUIMessageChunk[];
  finish: boolean;
}

/** 单信封 → 待投 chunk 组（观察点回调由调用侧先行触发，与投递解耦） */
function envelopeOutcome(state: StreamState, message: AgentRunMessage): EnvelopeOutcome {
  if (message.ipc === 'event') {
    return { chunks: frameChunks(state, message.event), finish: false };
  }
  if (message.ipc === 'record') {
    return { chunks: recordChunks(state, message.record), finish: true };
  }
  // 未知信封判别：忽略单条消息，流不断开
  return { chunks: [], finish: false };
}

/** 单事件 → chunk 组（帧 + 块）：`start` 每轮一次、`start-step` 每响应一次 */
function frameChunks(state: StreamState, event: AgentEvent): AgentUIMessageChunk[] {
  const messageId = event.turnId != null ? `turn-${event.turnId}` : `evt-${event.seq}`;
  const start = state.messageStarted ? [] : startChunks(state, messageId, event.seq);
  if (event.kind === 'messageDelta') return [...start, ...deltaFrame(state, event)];
  if (event.kind === 'message' && event.role === 'assistant') {
    return [...start, ...sealedFrame(state, event)];
  }
  // tool 结果就地补全（不开 step）/ user 提问（恒零 chunk）/ aux（单条 data 部件）
  return [...start, ...eventToChunk(event, state.currentToolCalls, anchorOf(state))];
}

/**
 * `Record` 收口：该轮消息尚未存在（零事件轮 / 旧转录 id 失配）时先发 `start`
 * 建消息，再以 `finish` 的 `messageMetadata` 挂轮行。`seq` **显式回填**——
 * `mergeObjects` 只跳过 `undefined`，`null` 会覆盖 `start` 写入的值。
 */
function recordChunks(state: StreamState, record: TurnSummary): AgentUIMessageChunk[] {
  const messageId = `turn-${record.turnId}`;
  const start = state.messageId === messageId ? [] : startChunks(state, messageId, state.startSeq);
  return [
    ...start,
    {
      type: 'finish',
      messageMetadata: { seq: state.startSeq, parentToolUseId: null, record },
    },
  ];
}

/** 订阅起点播种：登记「待播种轮 + 其既有密封事件」，等首个实时事件确认轮次
 * 后再预投。不在订阅建立时直接投，是因为订阅可能先收到别轮的事件（并发多轮 /
 * 旧轮残留），无条件预投会让两条 `start` 塌成一条消息。 */
interface PendingSeed {
  expect: (turnId: number | null, events: readonly AgentEvent[]) => void;
  seedIfMatching: (message: AgentRunMessage) => void;
}

function createPendingSeed(
  state: StreamState,
  controllerOf: () => ReadableStreamDefaultController<AgentUIMessageChunk> | null,
): PendingSeed {
  let pending: { turnId: number | null; events: readonly AgentEvent[] } | null = null;
  return {
    expect: (turnId, events) => {
      pending = { turnId, events };
    },
    seedIfMatching: (message) => {
      if (message.ipc !== 'event' || pending === null) return;
      if (pending.turnId !== message.event.turnId) return;
      const { events } = pending;
      pending = null;
      const controller = controllerOf();
      for (const seeded of events) {
        for (const chunk of frameChunks(state, seeded)) controller?.enqueue(chunk);
      }
    },
  };
}

/**
 * Tauri `ChatTransport` 实现：两出口同源同信封（`Channel<AgentRunMessage>`），
 * 发起期走 `agent_start`（Channel 先于 invoke，零空洞）、旁观 / 重挂期走
 * `agent_session_watch`（带 `since_seq`）；两出口共用 [`envelopeStream`] 的
 * 簿记单点与 `Record` 收口。
 */
export class TauriAgentTransport implements ChatTransport<AgentUIMessage> {
  private readonly onEvent?: (event: AgentEvent) => void;
  private readonly onRecord?: (record: TurnSummary) => void;
  private readonly observedEvents?: () => readonly AgentEvent[];

  constructor(options?: TauriAgentTransportOptions) {
    this.onEvent = options?.onEvent;
    this.onRecord = options?.onRecord;
    this.observedEvents = options?.observedEvents;
  }

  public async sendMessages(options: {
    trigger: 'submit-message' | 'regenerate-message';
    chatId: string;
    messageId: string | undefined;
    messages: AgentUIMessage[];
    abortSignal: AbortSignal | undefined;
    body?: object;
  }): Promise<ReadableStream<AgentUIMessageChunk>> {
    if (options.trigger !== 'submit-message') {
      throw new Error('TauriAgentTransport 仅支持 submit-message 触发');
    }
    const session = readSessionParams(options.body);
    const channel = new Channel<AgentRunMessage>();
    const envelope = this.envelopeStream(channel);
    // 发起流由 `Record` 收尾（invoke 提前 resolve 不关流）
    this.invokeStart(channel, session, envelope.fail);
    return envelope.stream;
  }

  /**
   * 订阅流：解析 body（`root` / `sessionId` / `sinceSeq`，非法或缺席 → null）→
   * 会话状态门（无 running 轮行 → null）→ 转录起步播种 → Channel + 簿记单点 →
   * invoke；invoke 解析（轮终态 / 断流 / 无条目）与 abort 均关流。
   *
   * 播种动因：`resume-stream` 在 ai-sdk 里以 `lastMessage: void 0` 起新消息状态，
   * 首个 chunk 的 `start` 命中「替换末条」→ 该轮由重放折叠出的部件会被增量独占
   * 覆盖（进行中会话「历史闪一下只剩新流」）。故订阅起点先把该会话已落库的密封
   * 事件按序预投进同一簿记，reducer 从空态重建出的就是「既有部件 + 增量」。
   */
  public async reconnectToStream(options: {
    chatId: string;
    abortSignal?: AbortSignal;
    body?: object;
  }): Promise<ReadableStream<AgentUIMessageChunk> | null> {
    const params = readStreamParams(options.body);
    if (params === null) return null;
    const { root, sessionId, sinceSeq } = params;
    const detail = await ipc.sessionDetail(root, sessionId);
    // 状态门：库态无 running 轮行即无实时面可续（已完成会话零订阅）
    if (detail === null || !detail.turns.some((turn) => turn.status === 'running')) {
      return null;
    }
    const running = detail.turns.find((turn) => turn.status === 'running');
    if (running === undefined) return null;
    const channel = new Channel<AgentRunMessage>();
    const envelope = this.envelopeStream(channel);
    // 起点播种：只投运行轮的、游标以内的密封事件（更早的轮属另一条 reducer
    // 消息；超出 sinceSeq 的由 watch 自己投，避免重复）。来源是消费方已装载的
    // 事件镜像——不额外取数
    envelope.expectSeed(
      running.turnId,
      (this.observedEvents?.() ?? []).filter(
        (event) =>
          event.turnId === running.turnId && event.kind !== 'messageDelta' && event.seq <= sinceSeq,
      ),
    );
    ipc
      .agentSessionWatch(channel, root, sessionId, sinceSeq)
      .then(envelope.end)
      .catch(envelope.fail);
    if (options.abortSignal?.aborted === true) {
      envelope.end();
    } else {
      options.abortSignal?.addEventListener('abort', envelope.end, { once: true });
    }
    return envelope.stream;
  }

  /**
   * step 分段簿记单点（两出口共用）：内部持有 `StreamState` 与 `closed`；
   * `accept` 分派事件 / `Record`，`end` 关流，`fail` 注错。
   */
  private envelopeStream(channel: Channel<AgentRunMessage>): EnvelopeStream {
    let controller: ReadableStreamDefaultController<AgentUIMessageChunk> | null = null;
    let closed = false;
    const state: StreamState = {
      messageStarted: false,
      messageId: null,
      startSeq: null,
      stepOpen: false,
      partAnchors: new Map(),
      currentToolCalls: new Set(),
    };
    const close = () => {
      if (closed) return;
      closed = true;
      controller?.close();
    };
    const fail = (cause: unknown) => {
      if (closed) return;
      closed = true;
      controller?.error(cause instanceof Error ? cause : new Error(String(cause)));
    };
    const pendingSeed = createPendingSeed(state, () => controller);
    const accept = (message: AgentRunMessage) => {
      if (closed) return;
      // 观察点先于投递（镜像与收口不依赖 chunk 消费进度）
      if (message.ipc === 'event') this.onEvent?.(message.event);
      else if (message.ipc === 'record') this.onRecord?.(message.record);
      pendingSeed.seedIfMatching(message);
      const outcome = envelopeOutcome(state, message);
      for (const chunk of outcome.chunks) controller?.enqueue(chunk);
      if (outcome.finish) close();
    };
    const stream = new ReadableStream<AgentUIMessageChunk>({
      start: (next) => {
        controller = next;
      },
      cancel: () => {
        // 消费侧取消（abort / 卸载）：后续信封一律丢弃
        closed = true;
      },
    });
    // Channel 信封入口在簿记单点内接线（调用方只负责 invoke）
    channel.onmessage = accept;
    return { stream, accept, end: close, fail, expectSeed: pendingSeed.expect };
  }

  /** 发起运行（生成绑定穿透会话参数 + Channel）：early-resolve 轮行经
   * `onRecord` 透传（不关流）；invoke 失败（启动阶段失败）把错误灌入流。
   * 参数以具名解构就位后按生成绑定的位置签名传递——尾位增位（如 `cwd`）不再
   * 依赖读参下标魔法，调用点即形状。 */
  private invokeStart(
    channel: Channel<AgentRunMessage>,
    session: AgentStartSessionParams,
    fail: (cause: unknown) => void,
  ): void {
    const { root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd } = session;
    ipc
      .agentStart(channel, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd)
      .then((record) => {
        this.onRecord?.(record);
      })
      .catch(fail);
  }
}
