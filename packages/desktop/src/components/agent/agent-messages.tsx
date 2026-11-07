import { memo, useCallback, useEffect, useRef, useState } from 'react';

import { Bubble, BubbleContent } from '@/components/ui/bubble';
import { Message, MessageContent } from '@/components/ui/message';
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';
import { Spinner } from '@/components/ui/spinner';
import { Trace } from '@/components/ui/trace';
import type { AgentToolPart, AgentUIMessage } from '@/lib/agent-adapter';
import { asRecord, textOf } from '@/lib/parse';
import type { AgentCitation, TurnSummary } from '@/types/dto';

import { TextBlock, ThinkingBlock, ToolPairCard, isToolPart } from './agent-blocks';
import { BUILD_IN_TOOL_CARDS } from './tool-cards';

export interface AgentMessagesProps {
  /** 对话状态（历史重放装载 + 实时累积，共用本透镜；全量入参，system 过滤在组件内） */
  messages: AgentUIMessage[];
  /** 重放装载进行中 */
  loading?: boolean;
  /** 运行中标记：头部呈现流式状态 */
  running?: boolean;
}

const MESSAGE_PAGE_SIZE = 10;
/** 触顶预载区高度：距顶部该距离即装载历史页，无需滚到绝对顶端 */
const HISTORY_PRELOAD_MARGIN_PX = 300;

/** 工具部件 → 特化卡（注册表命中）或默认对卡（miss 回退） */
function ToolPartView({ part }: { part: AgentToolPart }): React.JSX.Element | null {
  const toolName = part.type.slice('tool-'.length);
  const output = part.state === 'output-available' ? part.output : null;
  const registered = BUILD_IN_TOOL_CARDS[toolName];
  if (registered !== undefined) {
    const Card = registered;
    return (
      <Card toolCallId={part.toolCallId} toolName={toolName} input={part.input} output={output} />
    );
  }
  return (
    <ToolPairCard name={toolName} toolCallId={part.toolCallId} input={part.input} output={output} />
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
function PartView({ part }: { part: AgentUIMessage['parts'][number] }): React.JSX.Element | null {
  if (part.type === 'text') return <TextBlock citations={citationsOfPart(part)} text={part.text} />;
  if (part.type === 'reasoning') return <ThinkingBlock text={part.text} state={part.state} />;
  if (isToolPart(part)) return <ToolPartView part={part} />;
  return null;
}

/** 可见部件：`text` / `reasoning` / `tool-<name>` 三类计入；
 * `data-*` 与 `step-start` 不计（实时态的空壳消息由此不产空气泡） */
function hasVisibleParts(message: AgentUIMessage): boolean {
  return message.parts.some(
    (part) => part.type === 'text' || part.type === 'reasoning' || isToolPart(part),
  );
}

/** 轮状态（唯一来源：该轮 assistant 消息的 `metadata.record.status`） */
function turnStatusOf(message: AgentUIMessage): TurnSummary['status'] | null {
  return message.metadata?.record?.status ?? null;
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
 * 「已停止」；其他 status 不渲染。`stopped` MUST NOT 触发顶部失败横幅
 * （`agent-error.ts` 的既有判据正确，本文件零改动该面）。
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
  return null;
}

/** 气泡：user / assistant 各成泡（user 靠右），部件按类型映射渲染；assistant
 * 无可见部件且该轮 failed / stopped 时以轮内状态行取代部件列表 */
function ChatBubbleBase({ message }: { message: AgentUIMessage }): React.JSX.Element {
  const isUser = message.role === 'user';
  const record = message.metadata?.record ?? null;
  const statusLine =
    !isUser && !hasVisibleParts(message) && record !== null ? (
      <TurnStatusLine message={message} record={record} />
    ) : null;
  return isUser ? (
    <Message data-testid="chat-bubble" data-role={message.role}>
      <MessageContent>
        <Bubble variant="tinted" className="w-full max-w-full">
          <BubbleContent className="w-full">
            {message.parts.map((part, index) => (
              <PartView key={index} part={part} />
            ))}
          </BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  ) : (
    <Message data-testid="chat-bubble" data-role={message.role}>
      <MessageContent>
        {statusLine}
        <Trace className="pl-3">
          {message.parts.map((part, index) => (
            <PartView key={index} part={part} />
          ))}
        </Trace>
      </MessageContent>
    </Message>
  );
}

/** store 每拍只重建当前流式消息、历史消息对象引用稳定 → 未变气泡整棵子树跳过。
 * 流式消息内部不另加 memo：其 part 外层每拍都是新引用，真正的昂贵工作由
 * TextBlock/ToolPairCard 等内层按值/嵌套引用挡住。 */
const ChatBubble = memo(ChatBubbleBase);

/**
 * 触顶预载观察：sentinel 进入视口上方预载区即触发装载。
 * root 必须绑消息滚动视口，rootMargin 才是相对该容器计算的“临近顶部”预载区；
 * remaining 变化（装页或入参增长）后重挂观察器，视口未填满时可持续自动装载。
 */
function useTopReachObserver(
  viewportRef: React.RefObject<HTMLDivElement | null>,
  sentinelRef: React.RefObject<HTMLDivElement | null>,
  remaining: number,
  onReachTop: () => void,
): void {
  useEffect(() => {
    const viewport = viewportRef.current;
    const sentinel = sentinelRef.current;
    if (remaining === 0 || viewport === null || sentinel === null) {
      return;
    }
    if (typeof IntersectionObserver === 'undefined') {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          onReachTop();
        }
      },
      { root: viewport, rootMargin: `${HISTORY_PRELOAD_MARGIN_PX}px 0px 0px 0px` },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [viewportRef, sentinelRef, remaining, onReachTop]);
}

/** 触顶状态行：还有更早 → 装载提示；已装满且超过一页 → 全量终态；短对话不呈现 */
function HistoryLoadStatus({ remaining }: { remaining: number }): React.JSX.Element | null {
  if (remaining > 0) {
    return (
      <div className="self-center text-xs text-muted-foreground" data-testid="history-load-hint">
        加载中…
      </div>
    );
  }
  return null;
}

function MessageContainer({
  messages,
  remaining,
  onReachTop,
}: {
  messages: AgentUIMessage[];
  /** 尚未渲染的更早消息条数（0 = 已到最早，不再观察触顶） */
  remaining: number;
  onReachTop: () => void;
}): React.JSX.Element {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const topSentinelRef = useRef<HTMLDivElement | null>(null);
  useTopReachObserver(viewportRef, topSentinelRef, remaining, onReachTop);
  return (
    <MessageScroller>
      <MessageScrollerViewport aria-label="对话消息" ref={viewportRef}>
        <MessageScrollerContent className="p-3">
          {messages.length === 0 && (
            <div className="text-muted-foreground" data-testid="conversation-empty">
              尚无对话。在下方输入以开始探索。
            </div>
          )}
          <div ref={topSentinelRef} className="h-2 w-full">
            <HistoryLoadStatus remaining={remaining} />
          </div>
          {messages.map((message) => (
            <MessageScrollerItem
              key={message.id}
              messageId={message.id}
              scrollAnchor={message.role === 'user'}
            >
              <ChatBubble message={message} />
            </MessageScrollerItem>
          ))}
        </MessageScrollerContent>
      </MessageScrollerViewport>
      <MessageScrollerButton />
    </MessageScroller>
  );
}

/** 历史分页窗口：默认仅渲染最新一页；触顶递增一页；换会话（头部消息变化）复位 */
function useHistoryWindow(allMessages: AgentUIMessage[]): {
  visible: AgentUIMessage[];
  remaining: number;
  loadMore: () => void;
} {
  const [pages, setPages] = useState(1);
  const [firstMessageId, setFirstMessageId] = useState<string | null>(null);

  useEffect(() => {
    const headId = allMessages[0]?.id ?? null;
    if (headId !== null && headId !== firstMessageId) {
      setPages(1);
      setFirstMessageId(headId);
    }
  }, [allMessages, firstMessageId]);

  // 可见性判据（**全文件唯一过滤点**，MUST NOT 在别处重复）：system 角色剔除
  // → 存在可见部件即保留 → 否则按**轮状态**判定空内容轮（`failed` / `stopped`
  // 呈状态行保留；`completed` / 轮行缺席不渲染空气泡）。`data-*` 与 `step-start`
  // 不计入可见部件。分页计数基于过滤后列表。
  const conversationMessages = allMessages.filter(
    (message) =>
      message.role !== 'system' &&
      (hasVisibleParts(message) ||
        turnStatusOf(message) === 'failed' ||
        turnStatusOf(message) === 'stopped'),
  );
  const remaining = Math.max(0, conversationMessages.length - pages * MESSAGE_PAGE_SIZE);

  // 函数式更新 + 上限钳制：避免 pages 闭包过期，重复触顶也幂等
  const loadMore = useCallback(() => {
    setPages((prev) => (prev * MESSAGE_PAGE_SIZE >= conversationMessages.length ? prev : prev + 1));
  }, [conversationMessages.length]);

  return {
    visible: conversationMessages.slice(-pages * MESSAGE_PAGE_SIZE),
    remaining,
    loadMore,
  };
}

export function AgentMessages({
  messages,
  loading,
  running,
}: AgentMessagesProps): React.JSX.Element {
  const { visible: visibleMessages, remaining, loadMore } = useHistoryWindow(messages);
  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end" scrollPreviousItemPeek={64}>
      <section
        className="flex min-h-0 max-h-screen flex-1 flex-col rounded-lg border border-border bg-card"
        data-testid="agent-messages"
      >
        {loading && visibleMessages.length === 0 ? (
          <div className="p-3 text-muted-foreground" data-testid="agent-messages-loading">
            会话还原中…
          </div>
        ) : (
          <MessageContainer
            messages={visibleMessages}
            remaining={remaining}
            onReachTop={loadMore}
          />
        )}
        {running && (
          <div
            className="p-3 flex items-center text-muted-foreground"
            data-testid="agent-messages-running"
          >
            <Spinner />
            运行中…
          </div>
        )}
      </section>
    </MessageScrollerProvider>
  );
}
