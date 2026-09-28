/**
 * 对话透镜（agent 会话页统一消费）：渲染入口过滤 system 角色消息（适配层
 * 为非 message 事件与终态 record 合成的载体消息不进对话呈现，全量呈现归
 * 保真透镜 AgentTimeline），仅 user / assistant 气泡（user 靠右）——text →
 * markdown、reasoning → 折叠、tool → 对卡（AskUserQuestion 经注册表呈静态
 * 卡）；滚动沿用 shadcn message-scroller 组合（消息状态留在应用层）。工具
 * 配对已在适配层收口（input 与 output 同住一个 tool 部件），视图层零扫描。
 */

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
  loading: boolean;
  /** 运行中标记：头部呈现流式状态 */
  running: boolean;
}

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

/** 滚动承载：message-scroller 组合（Provider/Scroller/Viewport/Content/Item + 到底按钮） */
function MessagesScroller({ messages }: { messages: AgentUIMessage[] }): React.JSX.Element {
  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end">
      <MessageScroller className="flex-1">
        <MessageScrollerViewport aria-label="对话消息">
          <MessageScrollerContent className="px-3 py-3">
            {messages.length === 0 && (
              <div className="text-muted-foreground" data-testid="conversation-empty">
                尚无对话。在下方输入以开始探索。
              </div>
            )}
            {messages.map((message) => (
              <MessageScrollerItem key={message.id} messageId={message.id}>
                <ChatBubble message={message} />
              </MessageScrollerItem>
            ))}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton direction="end" />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}

/**
 * 对话区（对话透镜）：渲染入口过滤 system 角色消息（仅 user / assistant 气泡
 * 进呈现；辅助行 / raw / record 等载体呈现归保真透镜 AgentTimeline）；shadcn
 * message-scroller 承载滚动（turn 锚定 + 流式跟随 + 到底按钮）；部件映射——
 * text → markdown 气泡、reasoning → 折叠、tool → 对卡（AskUserQuestion 静态卡）。
 */
export function AgentMessages({
  messages,
  loading,
  running,
}: AgentMessagesProps): React.JSX.Element {
  const visible = messages.filter((message) => message.role !== 'system');
  return (
    <section
      className="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-card"
      data-testid="agent-messages"
    >
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <h2 className="m-0 text-[15px]">对话</h2>
        {running && (
          <span className="text-xs text-muted-foreground" data-testid="conversation-running">
            运行中…
          </span>
        )}
      </div>
      {loading && visible.length === 0 ? (
        <div className="p-3 text-muted-foreground" data-testid="conversation-loading">
          会话还原中…
        </div>
      ) : (
        <MessagesScroller messages={visible} />
      )}
    </section>
  );
}
