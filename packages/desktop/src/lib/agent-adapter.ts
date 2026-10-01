import type { UIMessage, UIMessageChunk } from 'ai';

import type { AgentBlock, AgentEvent, AgentMessageRole, TurnSummary } from '../types/dto';

/** 消息元数据：seq 保真（record 合成消息为 null）与子代理归因字段 */
interface AgentMessageMetadata {
  seq: number | null;
  parentToolUseId: string | null;
}

/** data 部件载荷：源事件字段 + seq / timestampMs 保真 */
interface AgentRunStartedData {
  seq: number;
  timestampMs: number;
  model: string | null;
  sessionId: string | null;
  tools: string[];
  mcpServers: string[];
}

interface AgentSystemNoticeData {
  seq: number;
  timestampMs: number;
  subtype: string;
  payload: unknown;
}

interface AgentRunResultData {
  seq: number;
  timestampMs: number;
  subtype: string;
  isError: boolean;
  numTurns: number | null;
  durationMs: number | null;
  costUsd: number | null;
  usage: unknown;
  sessionId: string | null;
}

export interface AgentRawData {
  seq: number;
  timestampMs: number;
  eventType: string;
  rawJson: string;
}

/** 工具结果载荷（tool 部件 output 与 chunk output 同形） */
export interface AgentToolOutput {
  content: string;
  isError: boolean;
}

/** 五个 data 部件形状（`data-run-record` 的 data 即轮行 DTO 整行）。
 * 类型别名（非 interface）：ai 的 UIDataTypes 约束要求对 Record 隐式索引签名 */
type AgentDataParts = {
  'run-started': AgentRunStartedData;
  'system-notice': AgentSystemNoticeData;
  'run-result': AgentRunResultData;
  raw: AgentRawData;
  'run-record': TurnSummary;
};

/** 前端对话状态一等模型 */
export type AgentUIMessage = UIMessage<AgentMessageMetadata, AgentDataParts>;

/** 实时增量 chunk（transport 流的元素类型） */
export type AgentUIMessageChunk = UIMessageChunk<AgentMessageMetadata, AgentDataParts>;

/** tool 部件（tool 名动态未知，`tool-${string}` 命名） */
export type AgentToolPart =
  | {
      type: `tool-${string}`;
      toolCallId: string;
      state: 'input-available';
      input: unknown;
    }
  | {
      type: `tool-${string}`;
      toolCallId: string;
      state: 'output-available';
      input: unknown;
      output: AgentToolOutput;
    };

/** 密封事件（delta 以外全变体）：重放路径唯一消费面 */
type SealedAgentEvent = Exclude<AgentEvent, { kind: 'messageDelta' }>;

/** 非 message 事件的 data 部件 / chunk 公共形态（type + data） */
type AuxData =
  | { type: 'data-run-started'; data: AgentRunStartedData }
  | { type: 'data-system-notice'; data: AgentSystemNoticeData }
  | { type: 'data-run-result'; data: AgentRunResultData }
  | { type: 'data-raw'; data: AgentRawData };

type ToolUseBlock = Extract<AgentBlock, { kind: 'toolUse' }>;
type ToolResultBlock = Extract<AgentBlock, { kind: 'toolResult' }>;

/** 无主 toolResult 就地合成时的占位工具名 */
const ORPHAN_TOOL_NAME = '工具调用';

/** 消息 id：seq 保真的稳定 key（重放与实时同构锚点） */
function messageId(seq: number): string {
  return `evt-${seq}`;
}

/**
 * provisional 消息键：配对键派生（adapter 与 transport 共用锚点）。delta 先
 * 行累积于该键命名的 provisional 消息，密封 Message 到达以密封为准替换。
 */
export function provisionalMessageId(parentToolUseId: string | null): string {
  return `delta-${parentToolUseId ?? 'none'}`;
}

/**
 * delta 累积部件的稳定 part id（首 delta 开件、后续累积共用；text / thinking
 * 各自一条，与密封整块部件 id `t-<seq>-<n>` 无碰撞域）。
 */
export function deltaPartId(key: string, part: 'text' | 'thinking'): string {
  return part === 'text' ? `t-delta-${key}` : `r-delta-${key}`;
}

/** 同 id toolResult 收集（含跨消息；仅收 toolUse 已配对的 id，重复取首个） */
function collectToolResults(events: SealedAgentEvent[]): Map<string, ToolResultBlock> {
  const useIds = new Set<string>();
  for (const event of events) {
    if (event.kind !== 'message') continue;
    for (const block of event.blocks) {
      if (block.kind === 'toolUse') useIds.add(block.id);
    }
  }
  const paired = new Map<string, ToolResultBlock>();
  for (const event of events) {
    if (event.kind !== 'message') continue;
    for (const block of event.blocks) {
      if (block.kind === 'toolResult' && useIds.has(block.id) && !paired.has(block.id)) {
        paired.set(block.id, block);
      }
    }
  }
  return paired;
}

/** 非 message 事件 → 单个 data 部件载荷（未知事件 `data-raw` 原文透传） */
function auxEventData(event: Exclude<SealedAgentEvent, { kind: 'message' }>): AuxData {
  const base = { seq: event.seq, timestampMs: event.timestampMs };
  switch (event.kind) {
    case 'runStarted':
      return {
        type: 'data-run-started',
        data: {
          ...base,
          model: event.model,
          sessionId: event.sessionId,
          tools: event.tools,
          mcpServers: event.mcpServers,
        },
      };
    case 'systemNotice':
      return {
        type: 'data-system-notice',
        data: { ...base, subtype: event.subtype, payload: event.payload },
      };
    case 'turnDone':
      return {
        type: 'data-run-result',
        data: {
          ...base,
          subtype: event.subtype,
          isError: event.isError,
          numTurns: event.numTurns,
          durationMs: event.durationMs,
          costUsd: event.costUsd,
          usage: event.usage,
          sessionId: event.sessionId,
        },
      };
    case 'raw':
      return {
        type: 'data-raw',
        data: { ...base, eventType: event.eventType, rawJson: event.rawJson },
      };
    default:
      // 运行时未知事件（未来引擎新增）：整事件 JSON 透传，不解释不丢失
      return {
        type: 'data-raw',
        data: { ...base, eventType: 'unknown', rawJson: JSON.stringify(event) },
      };
  }
}

/** toolUse 块 → tool 部件（有主结果并入 output，成单部件呈现） */
function toolUsePart(block: ToolUseBlock, results: Map<string, ToolResultBlock>): AgentToolPart {
  const result = results.get(block.id);
  if (result === undefined) {
    return {
      type: `tool-${block.name}`,
      toolCallId: block.id,
      state: 'input-available',
      input: block.input,
    };
  }
  return {
    type: `tool-${block.name}`,
    toolCallId: block.id,
    state: 'output-available',
    input: block.input,
    output: { content: result.content, isError: result.isError },
  };
}

/** message 事件块 → UIMessage 部件（四变体映射，已配对 result 不重复呈现） */
function blocksToParts(
  blocks: AgentBlock[],
  results: Map<string, ToolResultBlock>,
): AgentUIMessage['parts'] {
  const parts: AgentUIMessage['parts'] = [];
  for (const block of blocks) {
    if (block.kind === 'text') {
      parts.push({ type: 'text', text: block.text, state: 'done' });
    } else if (block.kind === 'thinking') {
      parts.push({ type: 'reasoning', text: block.thinking, state: 'done' });
    } else if (block.kind === 'toolUse') {
      parts.push(toolUsePart(block, results));
    } else if (!results.has(block.id)) {
      // 无主 toolResult：就地合成占位部件（有主已在 toolUse 侧并入）
      parts.push({
        type: `tool-${ORPHAN_TOOL_NAME}`,
        toolCallId: block.id,
        state: 'output-available',
        input: null,
        output: { content: block.content, isError: block.isError },
      });
    }
  }
  return parts;
}

/**
 * `tool`（引擎工具结果管道，非对话言语）落 `system`——ai-sdk UIMessage 无 tool role
 */
function uiRole(role: AgentMessageRole): AgentUIMessage['role'] {
  if (role === 'tool') return 'system';
  return role;
}

/** 单事件 → UIMessage（message 事件按 role 收窄；其余 system 角色单 data 部件） */
function eventToUIMessage(
  event: SealedAgentEvent,
  results: Map<string, ToolResultBlock>,
): AgentUIMessage {
  const metadata: AgentMessageMetadata = {
    seq: event.seq,
    parentToolUseId: event.kind === 'message' ? event.parentToolUseId : null,
  };
  if (event.kind === 'message') {
    return {
      id: messageId(event.seq),
      role: uiRole(event.role),
      metadata,
      parts: blocksToParts(event.blocks, results),
    };
  }
  return { id: messageId(event.seq), role: 'system', metadata, parts: [auxEventData(event)] };
}

export function eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[] {
  const sealed = events.filter((event): event is SealedAgentEvent => event.kind !== 'messageDelta');
  const results = collectToolResults(sealed);
  return sealed.map((event) => eventToUIMessage(event, results));
}

/**
 * 终态同构：轮统计行 → `data-run-record` 部件消息（重放与实时两路共用；
 * metadata.seq 为 null）。续轮（取当前会话）从中取。
 */
export function runRecordToUIMessage(record: TurnSummary): AgentUIMessage {
  return {
    id: `turn-${record.turnId}`,
    role: 'system',
    metadata: { seq: null, parentToolUseId: null },
    parts: [{ type: 'data-run-record', data: record }],
  };
}

/** text 块 → 整块 text chunk 组（无 token 拆分） */
function textChunks(id: string, text: string): AgentUIMessageChunk[] {
  return [
    { type: 'text-start', id },
    { type: 'text-delta', id, delta: text },
    { type: 'text-end', id },
  ];
}

/** thinking 块 → reasoning chunk 组 */
function reasoningChunks(id: string, thinking: string): AgentUIMessageChunk[] {
  return [
    { type: 'reasoning-start', id },
    { type: 'reasoning-delta', id, delta: thinking },
    { type: 'reasoning-end', id },
  ];
}

/** toolOutput chunk 载荷 */
function outputChunk(toolCallId: string, output: AgentToolOutput): AgentUIMessageChunk {
  return { type: 'tool-output-available', toolCallId, output };
}

/** message 事件块 → chunk 组（同事件配对在 toolUse 位就地续 output） */
function blocksToChunks(seq: number, blocks: AgentBlock[]): AgentUIMessageChunk[] {
  const useBlocks = blocks.filter((b): b is ToolUseBlock => b.kind === 'toolUse');
  const resultBlocks = blocks.filter((b): b is ToolResultBlock => b.kind === 'toolResult');
  const useIds = new Set(useBlocks.map((b) => b.id));
  const chunks: AgentUIMessageChunk[] = [];
  blocks.forEach((block, index) => {
    const key = `${seq}-${index}`;
    if (block.kind === 'text') {
      chunks.push(...textChunks(`t-${key}`, block.text));
    } else if (block.kind === 'thinking') {
      chunks.push(...reasoningChunks(`r-${key}`, block.thinking));
    } else if (block.kind === 'toolUse') {
      chunks.push({
        type: 'tool-input-available',
        toolCallId: block.id,
        toolName: block.name,
        input: block.input,
      });
      const result = resultBlocks.find((b) => b.id === block.id);
      if (result !== undefined) {
        chunks.push(outputChunk(block.id, { content: result.content, isError: result.isError }));
      }
    } else if (!useIds.has(block.id)) {
      // 无主 / 跨消息 toolResult：就地合成占位部件的 chunk 组（reducer 只能
      // 更新当前消息部件，跨消息并入由轮收口归一承载）
      chunks.push({
        type: 'tool-input-available',
        toolCallId: block.id,
        toolName: ORPHAN_TOOL_NAME,
        input: null,
      });
      chunks.push(outputChunk(block.id, { content: block.content, isError: block.isError }));
    }
  });
  return chunks;
}

/**
 * 实时增量：单事件 → chunk 组。
 */
export function eventToChunk(event: AgentEvent): AgentUIMessageChunk[] {
  if (event.kind === 'messageDelta') {
    const key = event.parentToolUseId ?? '';
    if (event.delta.kind === 'text') {
      return [{ type: 'text-delta', id: deltaPartId(key, 'text'), delta: event.delta.text }];
    }
    return [
      {
        type: 'reasoning-delta',
        id: deltaPartId(key, 'thinking'),
        delta: event.delta.thinking,
      },
    ];
  }
  const metadata: AgentMessageMetadata = {
    seq: event.seq,
    parentToolUseId: event.kind === 'message' ? event.parentToolUseId : null,
  };
  const header: AgentUIMessageChunk[] = [
    { type: 'start', messageId: messageId(event.seq), messageMetadata: metadata },
    { type: 'reset-step' },
  ];
  if (event.kind === 'message') {
    return [...header, ...blocksToChunks(event.seq, event.blocks)];
  }
  return [...header, auxEventData(event)];
}
