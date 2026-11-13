import type { ProviderMetadata, UIMessage, UIMessageChunk } from 'ai';

import type {
  AgentBlock,
  AgentCitation,
  AgentEvent,
  TurnSummary,
  WebSearchSource,
} from '../types/dto';

/**
 * 消息元数据：`seq` 保真（record 合成消息为 null）、子代理归因字段，以及
 * **回合元数据**载体 `record`（status / error / 统计）。
 *
 * `record` 取代既有轮行的 data 部件载体（`run-record` 成员已退役）：实时路由
 * `finish` chunk 的 `messageMetadata` 写入，折叠路由轮行匹配写入。`null` = 该轮
 * 无轮行且无 `TurnDone`；`undefined` = 实时流尚未收口。
 */
interface AgentMessageMetadata {
  seq: number | null;
  parentToolUseId: string | null;
  record?: TurnSummary | null;
}

/**
 * 事件载荷镜像的唯一派生器：`kind` 不进 data 部件（部件类型已表达）、
 * `turnId` 由消息级 `metadata` 承载。信封加变体或加字段时由 `Extract` /
 * `Omit` 编译期强制收敛（MUST NOT 保留无生成物守卫的手写副本）。
 */
type EventPayload<K extends AgentEvent['kind']> = Omit<
  Extract<AgentEvent, { kind: K }>,
  'kind' | 'turnId'
>;

/** data 部件载荷：源事件字段 + seq / timestampMs 保真 */
type AgentRunStartedData = EventPayload<'runStarted'>;

type AgentSystemNoticeData = EventPayload<'systemNotice'>;

type AgentRunResultData = EventPayload<'turnDone'>;

type AgentRawData = EventPayload<'raw'>;

/** 工具结果载荷（tool 部件 output 与 chunk output 同形）。`search` 为加位可选
 * ——云端 `web_search` 服务端工具的结构化载荷（既有消费面零改） */
export interface AgentToolOutput {
  content: string;
  isError: boolean;
  search?: { results: WebSearchSource[]; error: string | null };
}

/** 四个 data 部件形状（`run-record` 成员已退役：轮行改挂消息 metadata）。
 * 类型别名（非 interface）：ai 的 UIDataTypes 约束要求对 Record 隐式索引签名 */
type AgentDataParts = {
  'run-started': AgentRunStartedData;
  'system-notice': AgentSystemNoticeData;
  'run-result': AgentRunResultData;
  raw: AgentRawData;
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

/** 搜索来源载荷（result 侧块与视野配对的共同交付形态） */
interface SearchPayload {
  results: WebSearchSource[];
  error: string | null;
}

/** 块语义唯一表达（不含发射格式 / id / 视野）：`output: null` 即未配对。
 * text 变体携 `citations`（缺席 = 无引用）；tool 变体为**唯一工具类承载位**
 * ——客户端工具与云端 `web_search` 服务端工具共用（`toolName` 区分，
 * MUST NOT 为搜索新增第四变体） */
type BlockPlan =
  | { kind: 'text'; text: string; citations?: AgentCitation[] }
  | { kind: 'reasoning'; text: string }
  | {
      kind: 'tool';
      toolCallId: string;
      toolName: string;
      input: unknown;
      output: AgentToolOutput | null;
    };

/** 配对产物：工具结果块或搜索来源载荷的联合（与块种类同构；两视野同形态，
 * 差异只在产出方式） */
type PairedOutcome =
  | { kind: 'toolResult'; block: ToolResultBlock }
  | ({ kind: 'webSearch' } & SearchPayload);

/** 配对载体（id → 配对产物；重复结果取首个） */
type PairedResults = Map<string, PairedOutcome>;

/** 配对索引源：客户端工具调用 + 搜索 use 侧块（`rawUse != null` 即 use 侧） */
function collectUseIds(block: AgentBlock, useIds: Set<string>): void {
  if (block.kind === 'toolUse') {
    useIds.add(block.id);
  } else if (block.kind === 'webSearch' && block.rawUse != null) {
    useIds.add(block.id);
  }
}

/** 配对产物收录：同 id 的 `toolResult` 块或搜索 result 侧块（`rawUse == null`） */
function collectOutcome(block: AgentBlock, paired: PairedResults, useIds: Set<string>): void {
  if (block.kind === 'toolResult') {
    if (useIds.has(block.id) && !paired.has(block.id)) {
      paired.set(block.id, { kind: 'toolResult', block });
    }
  } else if (block.kind === 'webSearch' && block.rawUse == null) {
    if (useIds.has(block.id) && !paired.has(block.id)) {
      paired.set(block.id, {
        kind: 'webSearch',
        results: block.results ?? [],
        error: block.error ?? null,
      });
    }
  }
}

/** 视野层：配对视野差异的唯一隔离点（差异 MUST NOT 渗入语义层与发射层） */
const PairingScope = {
  /** 折叠路视野：跨消息预扫全部 use 侧块，再收其配对产物（重复取首个） */
  global: (events: SealedAgentEvent[]): PairedResults => {
    const useIds = new Set<string>();
    for (const event of events) {
      if (event.kind !== 'message') continue;
      for (const block of event.blocks) collectUseIds(block, useIds);
    }
    const paired: PairedResults = new Map();
    for (const event of events) {
      if (event.kind !== 'message') continue;
      for (const block of event.blocks) collectOutcome(block, paired, useIds);
    }
    return paired;
  },
  /** 实时路视野：仅当前 message 内 use ↔ 配对产物（跨消息不可见） */
  local: (blocks: AgentBlock[]): PairedResults => {
    const useIds = new Set<string>();
    for (const block of blocks) collectUseIds(block, useIds);
    const paired: PairedResults = new Map();
    for (const block of blocks) collectOutcome(block, paired, useIds);
    return paired;
  },
} as const;

/** 无主 toolResult 就地合成时的占位工具名 */
const ORPHAN_TOOL_NAME = '工具调用';

/** 云端 `web_search` 服务端工具的线名（工具卡注册表键同值） */
const WEB_SEARCH_TOOL_NAME = 'web_search';

/** 消息 id：seq 保真的稳定 key（aux 消息与旧转录兜底 id——`evt-<seq>`） */
function messageId(seq: number): string {
  return `evt-${seq}`;
}

/**
 * delta 累积部件的稳定 part id：由**该 step 内首个同类 delta 的 `seq`** 派生
 * （`t-<seq>` / `r-<seq>`）。同一 part 后续 delta 复用、跨 step 不撞
 * （`start-step` 时簿记清空）；与折叠路整块部件 id `t-<seq>-<n>` 无碰撞域。
 */
export function deltaPartId(seq: number, part: 'text' | 'thinking'): string {
  return part === 'text' ? `t-${seq}` : `r-${seq}`;
}

/**
 * delta part 锚解析回调：transport 传入持 per-step `partAnchors` 的实现；
 * 缺省实现返回入参 `seq`（缺省即逐 delta 独立 part）。以回调注入而非可变
 * Map 入参，使 `eventToChunk` 自身保持无状态（适配层「纯函数」纪律不破）。
 */
export type DeltaPartAnchor = (kind: 'text' | 'thinking', seq: number) => number;

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

/** 语义层窄化别名（块种类 → 变体类型；`planBlock` 分支收窄的唯一依赖面） */
type TextBlock = Extract<AgentBlock, { kind: 'text' }>;
type ToolUseBlock = Extract<AgentBlock, { kind: 'toolUse' }>;
type WebSearchBlock = Extract<AgentBlock, { kind: 'webSearch' }>;

/**
 * 语义层唯一判定点：块 → 计划（五变体 + 无主 result 合成；`null` = 已配对的
 * result 不重复呈现，其载荷已并入对应 use 侧计划）。
 */
function planBlock(block: AgentBlock, paired: PairedResults): BlockPlan | null {
  switch (block.kind) {
    case 'text':
      return textPlan(block);
    case 'thinking':
      return { kind: 'reasoning', text: block.thinking };
    case 'toolUse':
      return toolUsePlan(block, paired);
    case 'toolResult':
      return toolResultPlan(block, paired);
    case 'webSearch':
      return webSearchPlan(block, paired);
    default: {
      // 双层收口：编译期穷尽（新增 AgentBlock 变体而漏改即在此报错）+ runtime
      // 未知块变体跳过呈现（事件级未知仍由 `data-raw` 原文透传，不在此重复兜底）
      const _exhaustive: never = block;
      void _exhaustive;
      return null;
    }
  }
}

/** text 块 → text 计划（空 citations 折叠为缺席：契约上「无引用」两形态同义） */
function textPlan(block: TextBlock): BlockPlan {
  const citations = emptyAsUndefined(block.citations);
  return citations === undefined
    ? { kind: 'text', text: block.text }
    : { kind: 'text', text: block.text, citations };
}

/** toolUse 块 → tool 计划（客户端工具；配对命中则携 output） */
function toolUsePlan(block: ToolUseBlock, paired: PairedResults): BlockPlan {
  const outcome = paired.get(block.id);
  return {
    kind: 'tool',
    toolCallId: block.id,
    toolName: block.name,
    input: block.input,
    output:
      outcome?.kind === 'toolResult'
        ? { content: outcome.block.content, isError: outcome.block.isError }
        : null,
  };
}

/** toolResult 块 → tool 计划（已配对 = `null`；无主 result 合成占位工具计划，
 * 跨消息实时路的既有形态） */
function toolResultPlan(block: ToolResultBlock, paired: PairedResults): BlockPlan | null {
  if (paired.has(block.id)) return null;
  return {
    kind: 'tool',
    toolCallId: block.id,
    toolName: ORPHAN_TOOL_NAME,
    input: null,
    output: { content: block.content, isError: block.isError },
  };
}

/**
 * webSearch 块 → tool 计划（搜索复用工具卡管线，`toolName = 'web_search'`）。
 *
 * - use 侧（`rawUse != null`）：`input = { query }`；`output` 优先取**自身**
 *   `rawResult`（`pause_turn` 未拆分时 use 与 result 同处一条消息的收敛形态），
 *   其次取视野配对命中，两者皆无 = 未落定（`output: null`，只显查询词）；
 * - result 侧（`rawUse == null`）：配对命中返回 `null`（已被 use 消费），未命中
 *   产孤儿占位计划（`input: null` + 来源载荷，信息不丢）。
 */
function webSearchPlan(block: WebSearchBlock, paired: PairedResults): BlockPlan | null {
  const search: SearchPayload = { results: block.results ?? [], error: block.error ?? null };
  if (block.rawUse == null) {
    if (paired.has(block.id)) return null;
    return {
      kind: 'tool',
      toolCallId: block.id,
      toolName: WEB_SEARCH_TOOL_NAME,
      input: null,
      output: searchOutput(search),
    };
  }
  const own = block.rawResult == null ? null : search;
  const outcome = paired.get(block.id);
  const payload = own ?? (outcome?.kind === 'webSearch' ? outcome : null);
  return {
    kind: 'tool',
    toolCallId: block.id,
    toolName: WEB_SEARCH_TOOL_NAME,
    input: { query: block.query },
    output: payload === null ? null : searchOutput(payload),
  };
}

/** 空数组折叠为 `undefined`（citations 缺席与空同义：不挂 metadata、不渲染列表） */
function emptyAsUndefined<T>(items: T[] | null | undefined): T[] | undefined {
  return items == null || items.length === 0 ? undefined : items;
}

/** 搜索载荷 → `AgentToolOutput` 加位形态（`content` 位保留空串，`isError` =
 * 错误在场；结构化来源走 `search` 加位） */
function searchOutput(payload: SearchPayload): AgentToolOutput {
  return {
    content: '',
    isError: payload.error !== null,
    search: { results: payload.results, error: payload.error },
  };
}

/**
 * citations → text 部件 / chunk 的 `providerMetadata`（命名空间 `agent`——消费
 * 点在 provider 之外，MUST NOT 借 `anthropic` 名）：缺席 / 空数组 → `undefined`
 * （不挂空壳）；否则同一份 metadata 对象。
 *
 * 载荷形状为 `{ agent: { citations } }`——消费面（`agent-messages.tsx` 的
 * `citationsOfPart` / 呈现层）读 `providerMetadata.agent.citations`，故内层键名
 * MUST 为 `citations`（单层 `{ agent: citations }` 会让引用列表恒空）。
 *
 * `AgentCitation.raw` 在 TS 面为 `unknown`（serde_json::Value → specta
 * `unknown`），而 `ProviderMetadata` 要求 JSON 值域——值本身来自 IPC 的 JSON
 * 边界，故此处断言是**类型面补齐**而非运行时转换。metadata 合并语义为**覆盖
 * 非累加**，故 citations 只挂一次（`text-start` 与 `text-delta` 同一份对象）。
 */
function citationMetadata(citations: AgentCitation[] | undefined): ProviderMetadata | undefined {
  if (citations === undefined || citations.length === 0) return undefined;
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- 值来自 IPC 的 JSON 边界（raw 在 TS 面为 unknown），断言是类型面补齐而非运行时转换
  return { agent: { citations } } as unknown as ProviderMetadata;
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
      // 无引用时不落 `providerMetadata` 键（部件形状与变更前逐字一致）
      const metadata = citationMetadata(plan.citations);
      parts.push(
        metadata === undefined
          ? { type: 'text', text: plan.text, state: 'done' }
          : { type: 'text', text: plan.text, state: 'done', providerMetadata: metadata },
      );
    } else if (plan.kind === 'reasoning') {
      parts.push({ type: 'reasoning', text: plan.text, state: 'done' });
    } else {
      parts.push(planToolPart(plan));
    }
  }
  return parts;
}

/** 一轮的事件组：`turnId` 为主路径分组键，`null` 为旧转录（`RunStarted` 为组界） */
interface EventGroup {
  turnId: number | null;
  events: SealedAgentEvent[];
}

/** 输出项：一个事件组 + 该轮的轮行（库态权威；缺失时为合成行 / `null`） */
interface FoldEntry {
  group: EventGroup;
  record: TurnSummary | null;
}

type SealedMessage = Extract<SealedAgentEvent, { kind: 'message' }>;

/**
 * 单遍扫描分组（单套实现，两条分支同一循环）：`turnId != null` 按 `turnId`
 * 变化切组；`turnId == null` 的旧转录遇 `runStarted` 且当前组非空切组。
 * `RunStarted` 只作**旧转录组界**——真实运行事件恒有 `turnId`，故轮边界与
 * 轮计数 MUST NOT 依赖它（停止轮 / 失败轮 / 零事件轮可能看不到它）。
 */
function groupEvents(sealed: SealedAgentEvent[]): EventGroup[] {
  const groups: EventGroup[] = [];
  for (const event of sealed) {
    const current = groups.at(-1);
    const turnId = event.turnId ?? null;
    const opensGroup =
      current === undefined ||
      current.turnId !== turnId ||
      (turnId === null && event.kind === 'runStarted');
    if (opensGroup) {
      groups.push({ turnId, events: [event] });
    } else {
      current.events.push(event);
    }
  }
  return groups;
}

/**
 * 轮行缺席时的**内存合成行**（`TurnDone` 只兜 status，MUST NOT 作轮边界或轮行
 * 计数依据；`turnId = -1` 只活在内存，不落库、不作消息 id、不参与寻址）。
 * `TurnSummary` 无 `usage` 字段，`TurnDone.usage` 只留在 `data-run-result` 部件。
 */
function synthesizeRow(group: EventGroup): TurnSummary | null {
  const done = group.events.find((event) => event.kind === 'turnDone');
  if (done === undefined) return null;
  const first = group.events[0];
  return {
    turnId: done.turnId ?? -1,
    sessionId: done.sessionId ?? '',
    status: done.isError ? 'failed' : 'completed',
    startedAt: first === undefined ? done.timestampMs : first.timestampMs,
    finishedAt: done.timestampMs,
    numTurns: done.numTurns,
    costUsd: done.costUsd,
    durationMs: done.durationMs,
    error: null,
  };
}

/**
 * 输出序：轮行是库态权威（`SessionDetail.turns` 按发起序），故以其行序输出
 * ——零事件轮（ask 失败 / 立即停止）由此就位；`turnId` 为 null 的旧转录组
 * 按组序与轮行**逐位配对**（等价既有行为，不引入第二套分组）；未被任何行
 * 命中的组追加在行序之后（行缺失 → 合成行 / `null`）。
 */
function foldEntries(groups: EventGroup[], turns: TurnSummary[]): FoldEntry[] {
  const byTurn = new Map<number, EventGroup>();
  for (const group of groups) {
    if (group.turnId !== null && !byTurn.has(group.turnId)) byTurn.set(group.turnId, group);
  }
  const legacy = groups.filter((group) => group.turnId === null);
  const matched = new Set<EventGroup>();
  const entries: FoldEntry[] = [];
  let legacyIndex = 0;
  for (const row of turns) {
    const group = byTurn.get(row.turnId);
    if (group !== undefined) {
      matched.add(group);
      entries.push({ group, record: row });
      continue;
    }
    const legacyGroup = legacy[legacyIndex];
    if (legacyGroup === undefined) {
      entries.push({ group: { turnId: row.turnId, events: [] }, record: row });
      continue;
    }
    matched.add(legacyGroup);
    legacyIndex += 1;
    entries.push({ group: legacyGroup, record: row });
  }
  for (const group of groups) {
    if (!matched.has(group)) entries.push({ group, record: synthesizeRow(group) });
  }
  return entries;
}

/** 该轮 assistant 消息自身首事件（首个 assistant / tool 密封事件；提问块不计） */
function firstOwnEvent(group: EventGroup): SealedMessage | undefined {
  return group.events.find(
    (event): event is SealedMessage => event.kind === 'message' && event.role !== 'user',
  );
}

/**
 * 该轮 assistant 消息 id：主路径 `turn-<turnId>`；旧转录兜 `evt-<该消息首事件 seq>`
 * （与提问消息 `evt-<提问 seq>` 不同源，故两者 id 不撞）。
 *
 * 旧转录组若**无** assistant / tool 密封事件（残缺转录，如只有 `runStarted`），
 * 无「自身事件」可锚：退到组首事件 seq 并加 `-turn` 后缀，与同组的 aux 消息
 * `evt-<seq>` 及提问 `evt-<seq>` 均不撞（组内独立 React key）。
 */
function assistantId(group: EventGroup): string {
  if (group.turnId !== null) return `turn-${group.turnId}`;
  const own = firstOwnEvent(group);
  if (own !== undefined) return messageId(own.seq);
  const first = group.events[0];
  return `${messageId(first === undefined ? -1 : first.seq)}-turn`;
}

/** 该轮提问消息 id：`turn-<turnId>-user`（与 assistant 同族）；旧转录兜 `evt-<seq>`。
 * 只服务组内**首条** user 事件——其余 user 事件是该轮补充说明，id 锚在自身 seq。 */
function questionId(event: SealedMessage): string {
  return event.turnId != null ? `turn-${event.turnId}-user` : messageId(event.seq);
}

/** aux 事件 → 逐事件独立 system 消息（保真透镜 `AgentRawStream` 依赖逐事件回放） */
function auxMessage(event: Exclude<SealedAgentEvent, { kind: 'message' }>): AgentUIMessage {
  return {
    id: messageId(event.seq),
    role: 'system',
    metadata: { seq: event.seq, parentToolUseId: null },
    parts: [auxEventData(event)],
  };
}

/** 该轮提问事件 → user 消息（parts 为提问块直映射；可缺席） */
function questionMessage(event: SealedMessage, paired: PairedResults): AgentUIMessage {
  return {
    id: questionId(event),
    role: 'user',
    metadata: { seq: event.seq, parentToolUseId: event.parentToolUseId },
    parts: plansToParts(planBlocks(event.blocks, paired)),
  };
}

/**
 * 该轮补充说明事件 → user 消息（id 锚在自身 seq，与提问 id 不撞）。部件与提问
 * 共用同一条映射单点：后续 text 块携带 citations 或载荷加宽时两路不漂移。
 */
function steerMessage(event: SealedMessage, paired: PairedResults): AgentUIMessage {
  return {
    id: messageId(event.seq),
    role: 'user',
    metadata: { seq: event.seq, parentToolUseId: event.parentToolUseId },
    parts: plansToParts(planBlocks(event.blocks, paired)),
  };
}

/**
 * 该轮 assistant 消息：**恒一条且为该轮末条**（`isContinuation` 成立的前提）。
 * 部件 = 该轮全部 `role = assistant` 与 `role = tool` 密封事件的 blocks 顺序
 * 拼接（思考 / 正文 / 工具卡各占一 part，按首次到达顺序）；`parts` 可为空
 * （零内容轮）。`tool` 事件不产独立 system 消息，其配对结果就地补全工具部件。
 */
function assistantMessage(
  group: EventGroup,
  record: TurnSummary | null,
  paired: PairedResults,
): AgentUIMessage {
  const blocks: AgentBlock[] = [];
  // 子代理归因落**消息级单值**：取该轮首个 assistant 密封事件的归因
  const own = firstOwnEvent(group);
  const parentToolUseId = own === undefined ? null : own.parentToolUseId;
  for (const event of group.events) {
    if (event.kind !== 'message' || event.role === 'user') continue;
    blocks.push(...event.blocks);
  }
  const first = group.events[0];
  return {
    id: assistantId(group),
    role: 'assistant',
    metadata: {
      seq: first === undefined ? null : first.seq,
      parentToolUseId,
      record,
    },
    parts: plansToParts(planBlocks(blocks, paired)),
  };
}

/**
 * 单轮 → 消息序：aux system（按 seq 升序）→ 提问（可缺席）→ 补充说明（组内其余
 * user 事件，按 seq 序各一条）→ assistant（恒一条）。
 */
function entryToMessages(entry: FoldEntry, paired: PairedResults): AgentUIMessage[] {
  const messages: AgentUIMessage[] = [];
  for (const event of entry.group.events) {
    if (event.kind !== 'message') messages.push(auxMessage(event));
  }
  const users = entry.group.events.filter(
    (event): event is SealedMessage => event.kind === 'message' && event.role === 'user',
  );
  const [question, ...steers] = users;
  if (question !== undefined) messages.push(questionMessage(question, paired));
  for (const steer of steers) messages.push(steerMessage(steer, paired));
  messages.push(assistantMessage(entry.group, entry.record, paired));
  return messages;
}

/**
 * 重放 / 收口折叠唯一入口：只消费**密封事件**（转录中不存在 delta），按轮
 * 产出「提问（可缺席）+ 补充说明（组内其余 user 事件各一条，id `evt-<seq>`，
 * 序在提问之后、assistant 之前）+ 恰一条 assistant + 逐事件 aux system」，轮行
 * 挂该轮 assistant 的 `metadata.record`（按 `turnId` 匹配；缺失为合成行 / `null`）。
 */
export function foldSessionMessages(events: AgentEvent[], turns: TurnSummary[]): AgentUIMessage[] {
  const sealed = events.filter((event): event is SealedAgentEvent => event.kind !== 'messageDelta');
  // 折叠路视野：跨消息配对（重放 / 收口同一入口）
  const paired = PairingScope.global(sealed);
  return foldEntries(groupEvents(sealed), turns).flatMap((entry) => entryToMessages(entry, paired));
}

/** text 块 → 整块 text chunk 组（无 token 拆分）：`text-start` 与
 * `text-delta` 带同一份 metadata，`text-end` 不带 */
function textChunks(id: string, text: string, citations?: AgentCitation[]): AgentUIMessageChunk[] {
  const metadata = citationMetadata(citations);
  if (metadata === undefined) {
    return [
      { type: 'text-start', id },
      { type: 'text-delta', id, delta: text },
      { type: 'text-end', id },
    ];
  }
  // start / delta 共用同一份 metadata 对象（ai-sdk 的 metadata 合并为覆盖非累加）
  return [
    { type: 'text-start', id, providerMetadata: metadata },
    { type: 'text-delta', id, delta: text, providerMetadata: metadata },
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
      chunks.push(...textChunks(`t-${seq}-${index}`, plan.text, plan.citations));
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

/**
 * 实时增量：单事件 → **块 chunk** 组（delta 路 + 密封块路 + 工具就地补全）。
 *
 * 事件头与 step 帧（`start` / `start-step` / `reset-step` / `finish`）SHALL 归
 * transport，本函数 MUST NOT 产；`role = 'user'` 的密封事件零 chunk——chunk
 * 协议无 role 字段，提问气泡由会话状态侧承载。
 */
export function eventToChunk(
  event: AgentEvent,
  currentMessageToolCalls: ReadonlySet<string> = EMPTY_TOOL_CALLS,
  deltaAnchor: DeltaPartAnchor = (_kind, seq) => seq,
): AgentUIMessageChunk[] {
  if (event.kind === 'messageDelta') {
    const kind = event.delta.kind === 'text' ? 'text' : 'thinking';
    const id = deltaPartId(deltaAnchor(kind, event.seq), kind);
    return event.delta.kind === 'text'
      ? [{ type: 'text-delta', id, delta: event.delta.text }]
      : [{ type: 'reasoning-delta', id, delta: event.delta.thinking }];
  }
  if (event.kind === 'message') {
    // 提问气泡由会话状态侧产生（transport 与该侧共同保证「同一提问恰一条」）
    if (event.role === 'user') return [];
    const localUseIds = new Set<string>();
    for (const block of event.blocks) {
      if (block.kind === 'toolUse') localUseIds.add(block.id);
    }
    const merged = mergeableResults(event.blocks, currentMessageToolCalls, localUseIds);
    const rest = event.blocks.filter(
      (block) => !isConsumedResult(block, currentMessageToolCalls, localUseIds),
    );
    // 已配对结果先投 output（此刻 reducer 的当前消息仍是 toolUse 那条，就地补全），
    // 再为新块开件
    const mergedChunks = merged.map((block) =>
      outputChunk(block.id, { content: block.content, isError: block.isError }),
    );
    // 实时路视野：仅当前 message 内配对（跨消息差异由轮收口重放折叠纠正）
    const plans = planBlocks(rest, PairingScope.local(rest));
    return [...mergedChunks, ...plansToChunks(plans, event.seq)];
  }
  return [auxEventData(event)];
}
