import { memo, useCallback, useMemo } from 'react';

import { Message, MessageContent } from '@/components/ui/message';
import {
  type MessageScrollerGeometry,
  type MessageScrollerStickyEntry,
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerProvider,
  MessageScrollerSticky,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';
import { Trace } from '@/components/ui/trace';
import type { AgentToolPart, AgentUIMessage } from '@/lib/agent-adapter';
import {
  conversationIdentity,
  filterConversation,
  hasVisibleParts,
  userTurnSpans,
} from '@/lib/agent-virtual';
import { asRecord, textOf } from '@/lib/parse';
import type { AgentCitation, TurnSummary } from '@/types/dto';

import { TextBlock, ThinkingBlock, ToolPairCard, isToolPart } from './message-cards/blocks';
import { BUILD_IN_TOOL_CARDS } from './message-cards/tool-cards';
import { UserMessage } from './message-cards/user-message';

/** ask 应答动作面（三态判活的承载类型） */
export interface AskAnswerActions {
  onAnswer: (text: string) => void;
  /** 明确「当前正在等哪一条」（null = 当前无等待；undefined = 未提供，按未配对判活） */
  pendingToolCallId?: string | null;
}

export interface AgentMessagesProps {
  /** 对话状态（历史重放装载 + 实时累积，共用本透镜；全量入参，system 过滤在组件内） */
  messages: AgentUIMessage[];
  /** 重放装载进行中 */
  loading?: boolean;
  /** ask 应答动作面（缺席即全部 ask 卡静态只读） */
  askAnswer?: AskAnswerActions;
}

/** user 消息首帧估值：首次测量即校正，取「一行正文 + 气泡内边距」的量级 */
const USER_MESSAGE_ESTIMATE_PX = 72;
/** assistant 消息首帧估值：单条含部件列表，取整轮量级 */
const ASSISTANT_MESSAGE_ESTIMATE_PX = 240;
/** 行距：经虚拟器 `gap` 表达（内容容器不再用 flex `gap`） */
const MESSAGE_GAP_PX = 24;
/** 内容容器纵向内边距：经虚拟器 `paddingStart` / `paddingEnd` 表达 */
const MESSAGE_PADDING_PX = 12;
/** 可见窗口外扩项数：留一屏内的滑动余量即可 */
const VIRTUAL_OVERSCAN = 8;

/**
 * ask 部件是否活卡（三态判活矩阵，`pendingToolCallId` 精确对账优先）：
 * - `null`：当前无等待 → 全静态；
 * - 字符串：仅 `toolCallId` 与之相等者为活卡（历史转录里崩溃残留的未配对
 *   `tool_use` 同形，无 id 对账会误呈为可应答）；
 * - `undefined`：未提供对账 id → 由该轮是否仍在运行判活（`input-available`
 *   只作必要条件；MUST NOT 以结果缺席单独判活——中断 / 终止轮的未配对部件
 *   同样「没有结果」）。
 */
function isLiveAsk(
  part: AgentToolPart,
  pendingToolCallId: string | null | undefined,
  turnRunning: boolean | undefined,
): boolean {
  if (pendingToolCallId === null) return false;
  if (typeof pendingToolCallId === 'string') return part.toolCallId === pendingToolCallId;
  return part.state === 'input-available' && turnRunning !== false;
}

/** 工具部件 → 特化卡（注册表命中）或默认对卡（miss 回退） */
function ToolPartView({
  part,
  askAnswer,
  turnRunning,
}: {
  part: AgentToolPart;
  askAnswer?: AskAnswerActions;
  turnRunning?: boolean;
}): React.JSX.Element | null {
  const toolName = part.type.slice('tool-'.length);
  const output = part.state === 'output-available' ? part.output : null;
  const registered = BUILD_IN_TOOL_CARDS[toolName];
  if (registered !== undefined) {
    const Card = registered;
    // 动作面只达 ask 卡（其余三卡渲染与行为零变化）
    const cardAsk =
      askAnswer !== undefined &&
      toolName === 'ask_user_question' &&
      isLiveAsk(part, askAnswer.pendingToolCallId, turnRunning)
        ? { onAnswer: askAnswer.onAnswer }
        : undefined;
    return (
      <Card
        toolCallId={part.toolCallId}
        toolName={toolName}
        state={part.state}
        input={part.input}
        output={output}
        turnRunning={turnRunning}
        {...(cardAsk === undefined ? {} : { askAnswer: cardAsk })}
      />
    );
  }
  return (
    <ToolPairCard
      name={toolName}
      toolCallId={part.toolCallId}
      state={part.state}
      input={part.input}
      output={output}
    />
  );
}

/** 单条引用的宽解析（`raw` 原样透传；字段缺席 → `null`） */
function parseCitation(value: unknown): AgentCitation {
  const record = asRecord(value);
  return {
    url: textOf(record.url),
    title: textOf(record.title),
    citedText: textOf(record.citedText),
    raw: record.raw ?? null,
  };
}

/** text 部件的引用来源：`providerMetadata.agent.citations`（命名空间 `agent`，
 * 适配层挂载处；非数组 → `undefined`，空数组 → `undefined`——不渲染空容器） */
function citationsOfPart(part: AgentUIMessage['parts'][number]): AgentCitation[] | undefined {
  if (part.type !== 'text') return undefined;
  const citations = part.providerMetadata?.agent?.citations;
  if (!Array.isArray(citations)) return undefined;
  const parsed = citations.map(parseCitation);
  return parsed.length === 0 ? undefined : parsed;
}

/**
 * 气泡内部件分发（text / reasoning / tool）。非呈现部件返回 `null`：
 * `step-start` 是 reducer 的 step 标记（收口后由折叠整表替换消失），
 * `data-*` 是实时态 aux 载体（收口后归一到独立 system 消息）。
 */
function PartView({
  part,
  askAnswer,
  turnRunning,
}: {
  part: AgentUIMessage['parts'][number];
  askAnswer?: AskAnswerActions;
  turnRunning?: boolean;
}): React.JSX.Element | null {
  if (part.type === 'text') return <TextBlock citations={citationsOfPart(part)} text={part.text} />;
  if (part.type === 'reasoning') return <ThinkingBlock text={part.text} state={part.state} />;
  if (isToolPart(part)) {
    return <ToolPartView part={part} askAnswer={askAnswer} turnRunning={turnRunning} />;
  }
  return null;
}

/** 该消息内 `api_error` 通知的错误原文（实时态 aux 落本轮消息内；折叠态已归一
 * 为独立 system 消息，故该支在折叠态天然缺席） */
function noticeError(message: AgentUIMessage): string | null {
  for (const part of message.parts) {
    if (part.type !== 'data-system-notice') continue;
    if (part.data.subtype !== 'api_error') continue;
    const error = textOf(asRecord(part.data.payload).error);
    if (error !== null && error.trim() !== '') return error;
  }
  return null;
}

/** 状态行原文：库态权威（`record.error`）+ 该消息内 `api_error` 通知，去重拼接 */
function statusRawText(message: AgentUIMessage, record: TurnSummary): string[] {
  const lines: string[] = [];
  for (const line of [record.error, noticeError(message)]) {
    if (line === null || line.trim() === '') continue;
    if (!lines.includes(line)) lines.push(line);
  }
  return lines;
}

/**
 * 轮内状态行（`parts` 为空时）：「在发生的位置看到它」——正文是 content，
 * 运行事实是 provenance。`failed` 呈通用标题 + 可展开原文；`stopped` 呈
 * 「已停止」；`interrupted` 呈「因应用重启中断」（`completed` / 轮行缺席不渲染）。
 * `stopped` / `interrupted` MUST NOT 触发顶部失败横幅（`agent-error.ts` 的既有
 * 判据正确，本文件零改动该面）；`interrupted` 的标定记因是机器事实不是用户待
 * 处置故障，故不展开错误原文。
 */
function TurnStatusLine({
  message,
  record,
}: {
  message: AgentUIMessage;
  record: TurnSummary;
}): React.JSX.Element | null {
  if (record.status === 'failed') {
    const raw = statusRawText(message, record);
    return (
      <div
        className="my-1 text-sm text-destructive"
        data-testid="turn-status-line"
        data-status="failed"
      >
        <div data-testid="turn-status-line-failed">Agent 运行失败</div>
        {raw.length > 0 && (
          <details className="mt-1 text-xs text-muted-foreground">
            <summary>原始错误</summary>
            <div className="whitespace-pre-wrap break-all" data-testid="turn-status-line-raw">
              {raw.join('\n')}
            </div>
          </details>
        )}
      </div>
    );
  }
  if (record.status === 'stopped') {
    return (
      <div
        className="my-1 text-sm text-muted-foreground"
        data-testid="turn-status-line"
        data-status="stopped"
      >
        <div data-testid="turn-status-line-stopped">已停止</div>
      </div>
    );
  }
  if (record.status === 'interrupted') {
    return (
      <div
        className="my-1 text-sm text-muted-foreground"
        data-testid="turn-status-line"
        data-status="interrupted"
      >
        <div data-testid="turn-status-line-interrupted">因应用重启中断</div>
      </div>
    );
  }
  return null;
}

function AssistantMessageBase({
  message,
  askAnswer,
}: {
  message: AgentUIMessage;
  askAnswer?: AskAnswerActions;
}): React.JSX.Element {
  const record = message.metadata?.record ?? null;
  const turnRunning = record === null ? true : record.status === 'running';
  const statusLine =
    !hasVisibleParts(message) && record !== null ? (
      <TurnStatusLine message={message} record={record} />
    ) : null;
  return (
    <Message data-testid="chat-bubble" data-role={message.role}>
      <MessageContent>
        {statusLine}
        <Trace className="pl-3">
          {message.parts.map((part, index) => (
            <PartView key={index} part={part} askAnswer={askAnswer} turnRunning={turnRunning} />
          ))}
        </Trace>
      </MessageContent>
    </Message>
  );
}

const AssistantMessage = memo(AssistantMessageBase);

/**
 * 虚拟窗口的纵向几何：`gap` / `paddingStart` / `paddingEnd` 全走虚拟器，
 * 内容容器侧不再有 padding / flex gap——sticky 的夹取边界与绝对定位的包含块
 * 都是 padding box，`border-box` 下 padding 又计入 `height`，两处叠加会让末项
 * 越出容器、吸附层跨度与 `getTotalSize()` 账目不符。横向 gutter 落到行级 `px-3`。
 */
function MessageContainer({
  messages,
  resetKey,
  askAnswer,
}: {
  messages: AgentUIMessage[];
  resetKey: string | null;
  askAnswer?: AskAnswerActions;
}): React.JSX.Element {
  const estimateSize = useCallback(
    (index: number) =>
      messages[index]?.role === 'user' ? USER_MESSAGE_ESTIMATE_PX : ASSISTANT_MESSAGE_ESTIMATE_PX,
    [messages],
  );
  const getItemKey = useCallback((index: number) => messages[index]?.id ?? index, [messages]);
  const renderItem = useCallback(
    (index: number) => {
      const message = messages[index];
      if (message === undefined || message.role === 'user') return null;
      return <AssistantMessage message={message} askAnswer={askAnswer} />;
    },
    [messages, askAnswer],
  );
  const renderSticky = useCallback(
    (geometry: MessageScrollerGeometry): MessageScrollerStickyEntry[] =>
      userTurnSpans(messages, geometry.measurements, geometry.totalSize).flatMap(
        ({ index, start, span }) => {
          const message = messages[index];
          if (message === undefined) return [];
          return [
            {
              index,
              node: (
                <MessageScrollerSticky
                  key={message.id}
                  index={index}
                  start={start}
                  span={span - MESSAGE_GAP_PX}
                >
                  <UserMessage message={message} />
                </MessageScrollerSticky>
              ),
            },
          ];
        },
      ),
    [messages, askAnswer],
  );
  return (
    <MessageScrollerProvider
      count={messages.length}
      estimateSize={estimateSize}
      getItemKey={getItemKey}
      renderItem={renderItem}
      renderSticky={renderSticky}
      resetKey={resetKey}
      overscan={VIRTUAL_OVERSCAN}
      gap={MESSAGE_GAP_PX}
      paddingStart={MESSAGE_PADDING_PX}
      paddingEnd={MESSAGE_PADDING_PX}
    >
      <MessageScroller>
        <MessageScrollerViewport aria-label="对话消息">
          <MessageScrollerContent>
            {messages.length === 0 && (
              <div className="px-3 text-muted-foreground" data-testid="conversation-empty">
                尚无对话。在下方输入以开始探索。
              </div>
            )}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}

export function AgentMessages({
  messages,
  loading,
  askAnswer,
}: AgentMessagesProps): React.JSX.Element {
  const conversation = useMemo(() => filterConversation(messages), [messages]);
  const resetKey = useMemo(() => conversationIdentity(conversation), [conversation]);
  const onAnswer = askAnswer?.onAnswer;
  // 三态判活的第三态靠 `undefined`（未提供）与 `null`（当前无等待）可辨——此处
  // MUST NOT 用 `?? null` 归一，否则「未提供」会退化为「全静态」
  const pendingToolCallId = askAnswer?.pendingToolCallId;
  const stableAskAnswer = useMemo<AskAnswerActions | undefined>(
    () =>
      onAnswer === undefined
        ? undefined
        : pendingToolCallId === undefined
          ? { onAnswer }
          : { onAnswer, pendingToolCallId },
    [onAnswer, pendingToolCallId],
  );
  return (
    <section
      className="flex min-h-0 max-h-screen flex-1 flex-col rounded-lg"
      data-testid="agent-messages"
    >
      {loading && conversation.length === 0 ? (
        <div className="p-3 text-muted-foreground" data-testid="agent-messages-loading">
          会话还原中…
        </div>
      ) : (
        <MessageContainer messages={conversation} resetKey={resetKey} askAnswer={stableAskAnswer} />
      )}
    </section>
  );
}
