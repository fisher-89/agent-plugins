/**
 * Tauri 传输适配：ai-sdk v7 `ChatTransport` 的 Tauri IPC 实现（无状态转换器，
 * 不持链状态）。`sendMessages` 把 `options.body` 的链参数 + 提示词原样穿透
 * `invoke("agent_start")`，Tauri `Channel<AgentRunMessage>` 消息逐条翻译为
 * `UIMessageChunk` 流：Event 信封经 `eventToChunk` 转 chunk；Record 信封转
 * `data-run-record` 部件 + finish 后关流（终态由后端闭流收尾，前端流不在
 * 停止时截断）。`reconnectToStream` 恒 null（重连诉求由重放装载承担）。
 *
 * 链状态归应用层：hook 经 `onEvent` / `onRecord` 观测点维护 events / chain
 * 镜像；transport 只转换不解释。
 */

import { Channel, invoke } from '@tauri-apps/api/core';
import type { ChatTransport } from 'ai';

import type { AgentEvent, AgentPermissionMode, AgentRunRecord } from '../types/dto';
import { eventToChunk, type AgentUIMessage, type AgentUIMessageChunk } from './agent-adapter';

/** `agent_start` Channel 的消息信封（对齐 Rust serde tagged 线格式） */
type AgentRunMessage =
  | { ipc: 'event'; event: AgentEvent }
  | { ipc: 'record'; record: AgentRunRecord };

/** 链参数 + 提示词（hook 发送时刻组装，body 原样穿透） */
interface AgentStartChainParams {
  root: string;
  prompt: string;
  permissionMode: AgentPermissionMode;
  resumeSessionId: string | null;
  parentRunId: number | null;
  source: string;
  sourceRef: string | null;
}

/** transport 构造观测点：hook 注入（不承载状态，仅透传镜像） */
export interface TauriAgentTransportOptions {
  onEvent?: (event: AgentEvent) => void;
  onRecord?: (record: AgentRunRecord) => void;
}

/** unknown → AgentPermissionMode（清单外值拒绝，transport 不改写） */
function readPermissionMode(value: unknown): AgentPermissionMode {
  if (value === 'default' || value === 'acceptEdits' || value === 'bypassPermissions') {
    return value;
  }
  throw new Error(`非法 permissionMode: ${JSON.stringify(value)}`);
}

/** unknown → string | null（其余形态拒绝） */
function readNullableString(value: unknown, field: string): string | null {
  if (typeof value === 'string' || value === null) return value;
  throw new Error(`非法 ${field}: ${JSON.stringify(value)}`);
}

/** unknown → number | null（其余形态拒绝） */
function readNullableNumber(value: unknown, field: string): number | null {
  if (typeof value === 'number' || value === null) return value;
  throw new Error(`非法 ${field}: ${JSON.stringify(value)}`);
}

/** body 逐字段读取（运行时校验，transport 不增删改写链参数） */
function readChainParams(body: object | undefined): AgentStartChainParams {
  const get = (key: string): unknown =>
    Object.entries(body ?? {}).find(([name]) => name === key)?.[1];
  const root = get('root');
  const prompt = get('prompt');
  const source = get('source');
  if (typeof root !== 'string' || typeof prompt !== 'string' || typeof source !== 'string') {
    throw new Error('缺少 agent_start 链参数（root / prompt / source）');
  }
  return {
    root,
    prompt,
    source,
    permissionMode: readPermissionMode(get('permissionMode')),
    resumeSessionId: readNullableString(get('resumeSessionId'), 'resumeSessionId'),
    parentRunId: readNullableNumber(get('parentRunId'), 'parentRunId'),
    sourceRef: readNullableString(get('sourceRef'), 'sourceRef'),
  };
}

/**
 * Tauri `ChatTransport` 实现。发起新消息经 `agent_start` 提前 resolve
 * running 记录（经 `onRecord` 透传），事件与终态记录经 Channel 流入翻译为
 * chunk 流；Record 信封收尾（record 部件 + finish + 关流）。
 */
export class TauriAgentTransport implements ChatTransport<AgentUIMessage> {
  private readonly onEvent?: (event: AgentEvent) => void;
  private readonly onRecord?: (record: AgentRunRecord) => void;

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
    const chain = readChainParams(options.body);
    const channel = new Channel<AgentRunMessage>();
    return new ReadableStream<AgentUIMessageChunk>({
      start: (controller) => {
        let closed = false;
        channel.onmessage = (message) => {
          if (closed) return;
          if (message.ipc === 'event') {
            this.onEvent?.(message.event);
            for (const chunk of eventToChunk(message.event)) controller.enqueue(chunk);
            return;
          }
          if (message.ipc === 'record') {
            this.onRecord?.(message.record);
            controller.enqueue({
              type: 'start',
              messageId: `run-${message.record.id}`,
              messageMetadata: { seq: null, parentToolUseId: null },
            });
            controller.enqueue({ type: 'reset-step' });
            controller.enqueue({ type: 'data-run-record', data: message.record });
            controller.enqueue({ type: 'finish' });
            closed = true;
            controller.close();
            return;
          }
          // 未知信封判别：忽略单条消息，流不断开
        };
        invoke<AgentRunRecord>('agent_start', { ...chain, onEvent: channel })
          .then((record) => {
            this.onRecord?.(record);
          })
          .catch((cause: unknown) => {
            controller.error(cause instanceof Error ? cause : new Error(String(cause)));
          });
      },
    });
  }

  /** 重连由重放装载承担：恒 null */
  async reconnectToStream(): Promise<ReadableStream<AgentUIMessageChunk> | null> {
    return null;
  }
}
