import { Channel } from '@tauri-apps/api/core';
import type { ChatTransport } from 'ai';

import {
  commands,
  type AgentEvent,
  type AgentRunMessage,
  type AgentRunRecord,
} from '../types/generated/bindings';
import { eventToChunk, type AgentUIMessage, type AgentUIMessageChunk } from './agent-adapter';

/** `commands.agentStart` 位置参数面（生成绑定派生，无手写镜像） */
type AgentStartArgs = Parameters<typeof commands.agentStart>;

/** 链参数 + 提示词（hook 发送时刻组装，body 原样穿透；形状派生自生成绑定） */
type AgentStartChainParams = {
  root: AgentStartArgs[1];
  prompt: AgentStartArgs[2];
  permissionMode: AgentStartArgs[3];
  resumeSessionId: AgentStartArgs[4];
  source: AgentStartArgs[5];
  sourceRef: AgentStartArgs[6];
  parentRunId: AgentStartArgs[7];
  agent: AgentStartArgs[8];
};

/** transport 构造观测点：hook 注入（不承载状态，仅透传镜像） */
export interface TauriAgentTransportOptions {
  onEvent?: (event: AgentEvent) => void;
  onRecord?: (record: AgentRunRecord) => void;
}

/** unknown → AgentPermissionMode（清单外值拒绝，transport 不改写） */
function readPermissionMode(value: unknown): AgentStartChainParams['permissionMode'] {
  if (value === 'default' || value === 'acceptEdits' || value === 'bypassPermissions') {
    return value;
  }
  throw new Error(`非法 permissionMode: ${JSON.stringify(value)}`);
}

/** unknown → number | null（agent 可缺席——null / 缺席 → 后端解析默认 agent；
 * 清单外值拒绝，transport 不改写） */
function readAgentId(value: unknown): AgentStartChainParams['agent'] {
  if (value === undefined || value === null) return null;
  if (typeof value === 'number') return value;
  throw new Error(`非法 agent: ${JSON.stringify(value)}`);
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
    agent: readAgentId(get('agent')),
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
        this.invokeStart(channel, chain, controller);
      },
    });
  }

  /** 发起运行（生成绑定穿透链参数 + Channel）：early-resolve 记录经
   * `onRecord` 透传；invoke 失败（启动阶段失败）把错误灌入流。 */
  private invokeStart(
    channel: Channel<AgentRunMessage>,
    chain: AgentStartChainParams,
    controller: ReadableStreamDefaultController<AgentUIMessageChunk>,
  ): void {
    commands
      .agentStart(
        channel,
        chain.root,
        chain.prompt,
        chain.permissionMode,
        chain.resumeSessionId,
        chain.source,
        chain.sourceRef,
        chain.parentRunId,
        chain.agent,
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
