import { Channel } from '@tauri-apps/api/core';
import type { ChatTransport } from 'ai';

import {
  commands,
  type AgentDelta,
  type AgentEvent,
  type AgentRunMessage,
  type TurnSummary,
} from '../types/generated/bindings';
import {
  deltaPartId,
  eventToChunk,
  provisionalMessageId,
  type AgentUIMessage,
  type AgentUIMessageChunk,
} from './agent-adapter';

/** `commands.agentStart` 位置参数面（生成绑定派生，无手写镜像） */
type AgentStartArgs = Parameters<typeof commands.agentStart>;

/** 会话参数 + 提示词（hook 发送时刻组装，body 原样穿透；形状派生自生成绑定） */
type AgentStartSessionParams = {
  root: AgentStartArgs[1];
  prompt: AgentStartArgs[2];
  permissionMode: AgentStartArgs[3];
  sessionId: AgentStartArgs[4];
  source: AgentStartArgs[5];
  sourceRef: AgentStartArgs[6];
  agent: AgentStartArgs[7];
};

/** transport 构造观测点：hook 注入（不承载状态，仅透传镜像） */
export interface TauriAgentTransportOptions {
  onEvent?: (event: AgentEvent) => void;
  onRecord?: (record: TurnSummary) => void;
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

/** unknown → string | null（其余形态拒绝） */
function readNullableString(value: unknown, field: string): string | null {
  if (typeof value === 'string' || value === null) return value;
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
  };
}

/** 增量事件累积部件种类（思考/回复可辨；`messageDelta` 以外不适用） */
function deltaPartKind(delta: AgentDelta): 'text' | 'thinking' {
  return delta.kind === 'text' ? 'text' : 'thinking';
}

/** part-start chunk（text / reasoning 按 kind 各一，id 与 delta 累积同源） */
function partStartChunk(key: string, part: 'text' | 'thinking'): AgentUIMessageChunk {
  return part === 'text'
    ? { type: 'text-start', id: deltaPartId(key, 'text') }
    : { type: 'reasoning-start', id: deltaPartId(key, 'thinking') };
}

/** provisional 开件 chunk 组：start（配对键派生键，metadata 携源 seq）+
 * reset-step 让位 + part-start（首 delta 由 transport 流内簿记补发） */
function openProvisionalChunks(
  event: Extract<AgentEvent, { kind: 'messageDelta' }>,
): AgentUIMessageChunk[] {
  const key = event.parentToolUseId ?? '';
  return [
    {
      type: 'start',
      messageId: provisionalMessageId(event.parentToolUseId),
      messageMetadata: { seq: event.seq, parentToolUseId: event.parentToolUseId },
    },
    { type: 'reset-step' },
    partStartChunk(key, deltaPartKind(event.delta)),
  ];
}

/**
 * Tauri `ChatTransport` 实现
 */
export class TauriAgentTransport implements ChatTransport<AgentUIMessage> {
  private readonly onEvent?: (event: AgentEvent) => void;
  private readonly onRecord?: (record: TurnSummary) => void;

  constructor(options?: TauriAgentTransportOptions) {
    this.onEvent = options?.onEvent;
    this.onRecord = options?.onRecord;
  }

  async sendMessages(options: {
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
    return new ReadableStream<AgentUIMessageChunk>({ start: this.streamStart(channel, session) });
  }

  /** 流 start 回调装配：Channel 信封 → chunk 流（首 delta 簿记为流闭包内
   * 局部状态，随流存亡）；Record 信封收尾（record 部件 + finish + 关流）。 */
  private streamStart(
    channel: Channel<AgentRunMessage>,
    session: AgentStartSessionParams,
  ): (controller: ReadableStreamDefaultController<AgentUIMessageChunk>) => void {
    return (controller) => {
      let closed = false;
      /** 流内 delta 簿记：配对键 → 已开 part kind 集 */
      const openedParts = new Map<string, Set<'text' | 'thinking'>>();
      channel.onmessage = (message) => {
        if (closed) return;
        if (message.ipc === 'event') {
          this.onEvent?.(message.event);
          for (const chunk of this.eventChunks(message.event, openedParts)) {
            controller.enqueue(chunk);
          }
          return;
        }
        if (message.ipc === 'record') {
          this.onRecord?.(message.record);
          this.enqueueRecord(controller, message.record);
          closed = true;
          controller.close();
          return;
        }
        // 未知信封判别：忽略单条消息，流不断开
      };
      this.invokeStart(channel, session, controller);
    };
  }

  /** 单事件 → chunk 组：键首 delta 补发 provisional 开件组（start +
   * reset-step + part-start），同键新 kind 仅补 part-start（再发 reset-step
   * 会剪除本步已累积的另一种部件），同键同 kind 直发累积 chunk；密封/aux
   * 事件走 adapter 逐事件 chunk 组，簿记整体重武装。 */
  private eventChunks(
    event: AgentEvent,
    openedParts: Map<string, Set<'text' | 'thinking'>>,
  ): AgentUIMessageChunk[] {
    if (event.kind === 'messageDelta') {
      const key = event.parentToolUseId ?? '';
      const part = deltaPartKind(event.delta);
      const opened = openedParts.get(key);
      const chunks =
        opened === undefined
          ? openProvisionalChunks(event)
          : opened.has(part)
            ? []
            : [partStartChunk(key, part)];
      if (opened === undefined) {
        openedParts.set(key, new Set([part]));
      } else {
        opened.add(part);
      }
      return [...chunks, ...eventToChunk(event)];
    }
    // 密封/aux 事件经 start + reset-step 开新消息：reset-step 在 reducer 侧
    // 清空全部活跃 part，簿记随之整体清空——工具轮后模型再次流式时，裸 delta
    // 会因 part 已被清而抛 missing reasoning/text part
    openedParts.clear();
    return eventToChunk(event);
  }

  /** 终态轮行 → 同构 record 部件 chunk 组（与重放两路同构）。 */
  private enqueueRecord(
    controller: ReadableStreamDefaultController<AgentUIMessageChunk>,
    record: TurnSummary,
  ): void {
    controller.enqueue({
      type: 'start',
      messageId: `turn-${record.turnId}`,
      messageMetadata: { seq: null, parentToolUseId: null },
    });
    controller.enqueue({ type: 'reset-step' });
    controller.enqueue({ type: 'data-run-record', data: record });
    controller.enqueue({ type: 'finish' });
  }

  /** 发起运行（生成绑定穿透会话参数 + Channel）：early-resolve 轮行经
   * `onRecord` 透传；invoke 失败（启动阶段失败）把错误灌入流。 */
  private invokeStart(
    channel: Channel<AgentRunMessage>,
    session: AgentStartSessionParams,
    controller: ReadableStreamDefaultController<AgentUIMessageChunk>,
  ): void {
    commands
      .agentStart(
        channel,
        session.root,
        session.prompt,
        session.permissionMode,
        session.sessionId,
        session.source,
        session.sourceRef,
        session.agent,
      )
      .then((record) => {
        this.onRecord?.(record);
      })
      .catch((cause: unknown) => {
        controller.error(cause instanceof Error ? cause : new Error(String(cause)));
      });
  }

  /** 重连由重放装载承担：恒 null */
  async reconnectToStream(): Promise<ReadableStream<AgentUIMessageChunk> | null> {
    return null;
  }
}
