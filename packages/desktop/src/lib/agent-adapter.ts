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

interface AgentRawData {
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

type ToolResultBlock = Extract<AgentBlock, { kind: 'toolResult' }>;

/** 块语义唯一表达（不含发射格式 / id / 视野）：`output: null` 即未配对 */
type BlockPlan =
  | { kind: 'text'; text: string }
  | { kind: 'reasoning'; text: string }
  | {
      kind: 'tool';
      toolCallId: string;
      toolName: string;
      input: unknown;
      output: AgentToolOutput | null;
    };

/** 配对产物（两视野同形态，差异只在产出方式） */
type PairedResults = Map<string, ToolResultBlock>;

/** 视野层：配对视野差异的唯一隔离点（差异 MUST NOT 渗入语义层与发射层） */
const PairingScope = {
  /** 折叠路视野：跨消息预扫全部 toolUse，再收其配对 result（重复取首个） */
  global: (events: SealedAgentEvent[]): PairedResults => {
    const useIds = new Set<string>();
    for (const event of events) {
      if (event.kind !== 'message') continue;
      for (const block of event.blocks) {
        if (block.kind === 'toolUse') useIds.add(block.id);
      }
    }
    const paired: PairedResults = new Map();
    for (const event of events) {
      if (event.kind !== 'message') continue;
      for (const block of event.blocks) {
        if (block.kind === 'toolResult' && useIds.has(block.id) && !paired.has(block.id)) {
          paired.set(block.id, block);
        }
      }
    }
    return paired;
  },
  /** 实时路视野：仅当前 message 内 `toolUse` ↔ `toolResult` 配对（跨消息不可见） */
  local: (blocks: AgentBlock[]): PairedResults => {
    const useIds = new Set<string>();
    for (const block of blocks) {
      if (block.kind === 'toolUse') useIds.add(block.id);
    }
    const paired: PairedResults = new Map();
    for (const block of blocks) {
      if (block.kind === 'toolResult' && useIds.has(block.id) && !paired.has(block.id)) {
        paired.set(block.id, block);
      }
    }
    return paired;
  },
} as const;

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
    default: {
      // 双层收口：编译期穷尽（新增 AgentEvent 变体即报错）+ runtime 未知变体
      // `data-raw` 原文兜底（未来引擎新增事件不解释不丢失）
      const _exhaustive: never = event;
      return {
        type: 'data-raw',
        data: { ...base, eventType: 'unknown', rawJson: JSON.stringify(_exhaustive) },
      };
    }
  }
}

/**
 * 语义层唯一判定点：块 → 计划（四变体 + 无主 result 合成；`null` = 已配对的
 * result 不重复呈现，其载荷已并入对应 toolUse 计划）。
 */
function planBlock(block: AgentBlock, paired: PairedResults): BlockPlan | null {
  switch (block.kind) {
    case 'text':
      return { kind: 'text', text: block.text };
    case 'thinking':
      return { kind: 'reasoning', text: block.thinking };
    case 'toolUse': {
      const result = paired.get(block.id);
      return {
        kind: 'tool',
        toolCallId: block.id,
        toolName: block.name,
        input: block.input,
        output: result === undefined ? null : { content: result.content, isError: result.isError },
      };
    }
    case 'toolResult':
      if (paired.has(block.id)) return null;
      // 无主 result：合成占位工具计划（跨消息实时路的既有形态）
      return {
        kind: 'tool',
        toolCallId: block.id,
        toolName: ORPHAN_TOOL_NAME,
        input: null,
        output: { content: block.content, isError: block.isError },
      };
    default: {
      // 双层收口：编译期穷尽（新增 AgentBlock 变体而漏改即在此报错）+ runtime
      // 未知块变体跳过呈现（事件级未知仍由 `data-raw` 原文透传，不在此重复兜底）
      const _exhaustive: never = block;
      void _exhaustive;
      return null;
    }
  }
}

/** 块序列 → 计划序列（已配对 result 落空，不作占位） */
function planBlocks(blocks: AgentBlock[], paired: PairedResults): BlockPlan[] {
  const plans: BlockPlan[] = [];
  for (const block of blocks) {
    const plan = planBlock(block, paired);
    if (plan !== null) plans.push(plan);
  }
  return plans;
}

/** 工具计划 → tool 部件（未配对 = `input-available`，已配对 = `output-available`） */
function planToolPart(plan: Extract<BlockPlan, { kind: 'tool' }>): AgentToolPart {
  if (plan.output === null) {
    return {
      type: `tool-${plan.toolName}`,
      toolCallId: plan.toolCallId,
      state: 'input-available',
      input: plan.input,
    };
  }
  return {
    type: `tool-${plan.toolName}`,
    toolCallId: plan.toolCallId,
    state: 'output-available',
    input: plan.input,
    output: plan.output,
  };
}

/** 发射层（折叠路）：计划序列 → UIMessage 部件（纯机械，无判定无视野） */
function plansToParts(plans: BlockPlan[]): AgentUIMessage['parts'] {
  const parts: AgentUIMessage['parts'] = [];
  for (const plan of plans) {
    if (plan.kind === 'text') {
      parts.push({ type: 'text', text: plan.text, state: 'done' });
    } else if (plan.kind === 'reasoning') {
      parts.push({ type: 'reasoning', text: plan.text, state: 'done' });
    } else {
      parts.push(planToolPart(plan));
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
function eventToUIMessage(event: SealedAgentEvent, results: PairedResults): AgentUIMessage {
  const metadata: AgentMessageMetadata = {
    seq: event.seq,
    parentToolUseId: event.kind === 'message' ? event.parentToolUseId : null,
  };
  if (event.kind === 'message') {
    return {
      id: messageId(event.seq),
      role: uiRole(event.role),
      metadata,
      parts: plansToParts(planBlocks(event.blocks, results)),
    };
  }
  return { id: messageId(event.seq), role: 'system', metadata, parts: [auxEventData(event)] };
}

export function eventsToUIMessages(events: AgentEvent[]): AgentUIMessage[] {
  const sealed = events.filter((event): event is SealedAgentEvent => event.kind !== 'messageDelta');
  // 折叠路视野：跨消息配对（重放 / 收口同一入口）
  const results = PairingScope.global(sealed);
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

/** 空配对视野（`eventToChunk` 缺省参：既有本事件 `local` 语义） */
const EMPTY_TOOL_CALLS: ReadonlySet<string> = new Set();

/** 跨消息视野已消费的结果块（合并块及其重复 id 的后继块，均不再另起消息）；
 * 同事件内另有同 id toolUse 时让位本事件 `local` 配对 */
function isConsumedResult(
  block: AgentBlock,
  currentMessageToolCalls: ReadonlySet<string>,
  localUseIds: ReadonlySet<string>,
): block is ToolResultBlock {
  return (
    block.kind === 'toolResult' &&
    currentMessageToolCalls.has(block.id) &&
    !localUseIds.has(block.id)
  );
}

/**
 * 实时路跨消息视野：命中 `currentMessageToolCalls` 的 toolResult 块返回「首个」
 * （同 id 后续块视为重复结果，与折叠路同口径）。
 */
function mergeableResults(
  blocks: AgentBlock[],
  currentMessageToolCalls: ReadonlySet<string>,
  localUseIds: ReadonlySet<string>,
): ToolResultBlock[] {
  const seen = new Set<string>();
  const merged: ToolResultBlock[] = [];
  for (const block of blocks) {
    if (!isConsumedResult(block, currentMessageToolCalls, localUseIds)) continue;
    if (seen.has(block.id)) continue;
    seen.add(block.id);
    merged.push(block);
  }
  return merged;
}

/** message 事件块 → chunk 组（发射层实时路：纯机械，part id 取计划下标） */
function plansToChunks(plans: BlockPlan[], seq: number): AgentUIMessageChunk[] {
  const chunks: AgentUIMessageChunk[] = [];
  plans.forEach((plan, index) => {
    if (plan.kind === 'text') {
      chunks.push(...textChunks(`t-${seq}-${index}`, plan.text));
    } else if (plan.kind === 'reasoning') {
      chunks.push(...reasoningChunks(`r-${seq}-${index}`, plan.text));
    } else {
      chunks.push({
        type: 'tool-input-available',
        toolCallId: plan.toolCallId,
        toolName: plan.toolName,
        input: plan.input,
      });
      if (plan.output !== null) {
        chunks.push(outputChunk(plan.toolCallId, plan.output));
      }
    }
  });
  return chunks;
}

/** 实时增量：单事件 → chunk 组 */
export function eventToChunk(
  event: AgentEvent,
  currentMessageToolCalls: ReadonlySet<string> = EMPTY_TOOL_CALLS,
): AgentUIMessageChunk[] {
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
    const localUseIds = new Set<string>();
    for (const block of event.blocks) {
      if (block.kind === 'toolUse') localUseIds.add(block.id);
    }
    const merged = mergeableResults(event.blocks, currentMessageToolCalls, localUseIds);
    const rest = event.blocks.filter(
      (block) => !isConsumedResult(block, currentMessageToolCalls, localUseIds),
    );
    // 已配对结果先投 output（此刻 reducer 的当前消息仍是 toolUse 那条，就地补全），
    // 再为新块开件；全部块均就地补全时本事件不产消息头
    const mergedChunks = merged.map((block) =>
      outputChunk(block.id, { content: block.content, isError: block.isError }),
    );
    if (mergedChunks.length > 0 && rest.length === 0) return mergedChunks;
    // 实时路视野：仅当前 message 内配对（跨消息差异由轮收口重放折叠纠正）
    const plans = planBlocks(rest, PairingScope.local(rest));
    return [...mergedChunks, ...header, ...plansToChunks(plans, event.seq)];
  }
  return [...header, auxEventData(event)];
}
