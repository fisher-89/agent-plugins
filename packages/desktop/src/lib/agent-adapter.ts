/**
 * 事件适配层（纯函数）：AgentEvent 信封 → ai-sdk UIMessage / UIMessageChunk。
 *
 * 两条转换路（两路同构是硬不变量，spec：desktop-agent-chat-infra）：
 * - `eventsToUIMessages` 重放折叠：事件序列重建为 UIMessage 序列（重放装载
 *   与 run 结束后的状态归一共用）；
 * - `eventToChunk` 实时增量：单事件产出 chunk 组，经 useChat reducer 归约。
 *
 * 映射定稿（design）：runStarted → `data-run-started`；message.text → text
 * 部件（整块）；thinking → reasoning；toolUse → tool 部件 input；同 id
 * toolResult 并入该部件 output（配对收口于适配层，跨消息按 id）；无主
 * toolResult 就地合成占位部件；systemNotice → `data-system-notice`；
 * runResult → `data-run-result`；raw / 未知事件 → `data-raw` 原文透传。
 *
 * 保真锁定：seq 进 message id（`evt-<seq>`）与 message metadata（key 稳定、
 * 保序）；`parentToolUseId` 进 message metadata（子代理归因依赖）；未知事件
 * 一律 `data-raw` 透传不丢。
 *
 * 两路收敛口径：useChat reducer 单请求只产 assistant 角色消息，实时路的
 * 角色/配对呈现以 run 结束后的 `eventsToUIMessages` 归一为准（hook 在流收口
 * 时 setMessages 归一）；静止态两路产出同一状态形状。
 */

import type { UIMessage, UIMessageChunk } from 'ai';

import type { AgentBlock, AgentEvent, AgentRunRecord } from '../types/dto';

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

/** 五个 data 部件形状（`data-run-record` 的 data 即 AgentRunRecord 整行）。
 * 类型别名（非 interface）：ai 的 UIDataTypes 约束要求对 Record 隐式索引签名 */
type AgentDataParts = {
  'run-started': AgentRunStartedData;
  'system-notice': AgentSystemNoticeData;
  'run-result': AgentRunResultData;
  raw: AgentRawData;
  'run-record': AgentRunRecord;
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

/** 同 id toolResult 收集（含跨消息；仅收 toolUse 已配对的 id，重复取首个） */
function collectToolResults(events: AgentEvent[]): Map<string, ToolResultBlock> {
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
function auxEventData(event: Exclude<AgentEvent, { kind: 'message' }>): AuxData {
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
    case 'runResult':
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
      // 运行时未知事件（未来 CLI 新增）：整事件 JSON 透传，不解释不丢失
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

/** 单事件 → UIMessage（message 事件按 role 收窄；其余 system 角色单 data 部件） */
function eventToUIMessage(
  event: AgentEvent,
  results: Map<string, ToolResultBlock>,
): AgentUIMessage {
  const metadata: AgentMessageMetadata = {
    seq: event.seq,
    parentToolUseId: event.kind === 'message' ? event.parentToolUseId : null,
  };
  if (event.kind === 'message') {
    return {
      id: messageId(event.seq),
      role: event.role === 'user' ? 'user' : 'assistant',
      metadata,
      parts: blocksToParts(event.blocks, results),
    };
  }
  return { id: messageId(event.seq), role: 'system', metadata, parts: [auxEventData(event)] };
}

/**
 * 重放折叠：事件序列 → UIMessage 序列（顺序与 seq 一致）。工具同 id 配对
 * 就地收敛（input 与 output 同住一个 tool 部件），无主 result 合成占位
 * 部件，未知事件 `data-raw` 透传不丢。与 `eventToChunk` 流经 useChat 且
 * run 收口归一后的状态形状一致（两路同构）。
 */
export function eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[] {
  const results = collectToolResults(events);
  return events.map((event) => eventToUIMessage(event, results));
}

/**
 * 终态同构：run 记录 → `data-run-record` 部件消息（重放与实时两路共用；
 * metadata.seq 为 null）。链推进（续话取新链尾）从中取。
 */
export function runRecordToUIMessage(record: AgentRunRecord): AgentUIMessage {
  return {
    id: `run-${record.id}`,
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
      // 更新当前消息部件，跨消息并入由 run 收口归一承载）
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
 * 实时增量：单事件 → chunk 组。每组以 `start`（messageId = `evt-<seq>`，
 * metadata 携带 seq 与 `parentToolUseId`）开新消息、`reset-step` 把在编
 * 消息已累积的上一事件部件让位（先换 id 使旧消息成快照、再让位，已推送
 * 消息不受影响），再以部件 chunk 填充——逐组流入 useChat reducer 即得逐
 * 事件消息序列，化解 reducer「单请求单消息」的部件累积。非 message 事件
 * 产单个 data 部件 chunk。
 */
export function eventToChunk(event: AgentEvent): AgentUIMessageChunk[] {
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
