import { useCallback, useEffect, useRef, useState } from 'react';

import { Message, MessageContent } from '@/components/ui/message';
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';

import type { AgentToolPart, AgentUIMessage } from '../../lib/agent-adapter';
import { TextBlock, ThinkingBlock, ToolPairCard, isToolPart } from './agent-blocks';
import { DEFAULT_TOOL_CARDS } from './tool-cards';

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
  const registered = DEFAULT_TOOL_CARDS[toolName];
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

/** 气泡内部件分发（text / reasoning / tool；data 部件随 system 载体不进对话呈现） */
function PartView({ part }: { part: AgentUIMessage['parts'][number] }): React.JSX.Element | null {
  if (part.type === 'text') return <TextBlock text={part.text} />;
  if (part.type === 'reasoning') return <ThinkingBlock text={part.text} />;
  if (isToolPart(part)) return <ToolPartView part={part} />;
  return null;
}

/** 气泡：user / assistant 各成泡（user 靠右），部件按类型映射渲染 */
function ChatBubble({ message }: { message: AgentUIMessage }): React.JSX.Element {
  const isUser = message.role === 'user';
  return (
    <Message align={isUser ? 'end' : 'start'} data-testid="chat-bubble" data-role={message.role}>
      <MessageContent
        className={`rounded-lg border border-border px-3 py-2 ${isUser ? 'bg-muted' : 'bg-card'}`}
      >
        <div className="mb-0.5 text-xs text-muted-foreground">{message.role}</div>
        {message.parts.map((part, index) => (
          <PartView key={index} part={part} />
        ))}
      </MessageContent>
    </Message>
  );
}

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
function HistoryLoadStatus({
  remaining,
  loadedCount,
}: {
  remaining: number;
  loadedCount: number;
}): React.JSX.Element | null {
  if (remaining > 0) {
    return (
      <div className="self-center text-xs text-muted-foreground" data-testid="history-load-hint">
        滚动到顶部加载更早消息（还有 {remaining} 条）
      </div>
    );
  }
  if (loadedCount > MESSAGE_PAGE_SIZE) {
    return (
      <div
        className="self-center text-xs text-muted-foreground"
        data-testid="history-load-complete"
      >
        已加载全部 {loadedCount} 条消息
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
          <HistoryLoadStatus remaining={remaining} loadedCount={messages.length} />
          <div ref={topSentinelRef} className="h-2 w-full" />
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

  const conversationMessages = allMessages.filter((message) => message.role !== 'system');
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
    <MessageScrollerProvider autoScroll defaultScrollPosition="end">
      <section
        className="flex min-h-0 max-h-screen flex-1 flex-col rounded-lg border border-border bg-card"
        data-testid="agent-messages"
      >
        <div className="flex items-center gap-2 border-b border-border px-3 py-2">
          <h2 className="m-0 text-[15px]">对话</h2>
          {running && (
            <span className="text-xs text-muted-foreground" data-testid="agent-messages-running">
              运行中…
            </span>
          )}
        </div>
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
      </section>
    </MessageScrollerProvider>
  );
}
