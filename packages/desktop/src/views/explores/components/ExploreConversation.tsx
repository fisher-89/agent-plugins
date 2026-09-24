import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { Message, MessageContent } from '@/components/ui/message';
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';

import type { AgentBlock, AgentEvent } from '../../../types/dto';

export interface ExploreConversationProps {
  /** 对话事件（历史链重放 + 实时累积，共用本组件） */
  events: AgentEvent[];
  /** 链还原 / 重放进行中 */
  loading: boolean;
  /** 运行中标记：头部呈现流式状态 */
  running: boolean;
}

type MessageEvent = Extract<AgentEvent, { kind: 'message' }>;
type RunResultEvent = Extract<AgentEvent, { kind: 'runResult' }>;
type ToolUseBlock = Extract<AgentBlock, { kind: 'toolUse' }>;
type ToolResultBlock = Extract<AgentBlock, { kind: 'toolResult' }>;

/** 气泡 / 辅助行统一条目（seq 保序，key 稳定） */
type Entry =
  | { kind: 'chat'; seq: number; event: MessageEvent }
  | { kind: 'info'; seq: number; text: string }
  | { kind: 'result'; seq: number; event: RunResultEvent }
  | { kind: 'raw'; seq: number; eventType: string; rawJson: string };

/** 事件流 → 条目序列（message 成气泡，其余辅助行 / 汇总卡 / 原文透传） */
function toEntries(events: AgentEvent[]): Entry[] {
  return events.map((event): Entry => {
    if (event.kind === 'message') return { kind: 'chat', seq: event.seq, event };
    if (event.kind === 'runResult') return { kind: 'result', seq: event.seq, event };
    if (event.kind === 'raw')
      return { kind: 'raw', seq: event.seq, eventType: event.eventType, rawJson: event.rawJson };
    if (event.kind === 'systemNotice')
      return { kind: 'info', seq: event.seq, text: `system 通知（${event.subtype}）` };
    return { kind: 'info', seq: event.seq, text: 'run 启动' };
  });
}

/** 全局收集 ToolResult（含跨消息），供同 id ToolUse 成对；重复 id 取首个 */
function collectResults(events: AgentEvent[]): Map<string, ToolResultBlock> {
  const paired = new Map<string, ToolResultBlock>();
  for (const event of events) {
    if (event.kind !== 'message') continue;
    for (const block of event.blocks) {
      if (block.kind === 'toolResult' && !paired.has(block.id)) paired.set(block.id, block);
    }
  }
  return paired;
}

/** AskUserQuestion 静态卡的问题/选项形态（宽容解析，未知形态回退 JSON 原样） */
interface AskOption {
  label: string;
  description: string | null;
}
interface AskQuestion {
  question: string;
  options: AskOption[];
}

/** 宽松对象化：非对象一律空对象（免断言的未知形态收敛） */
function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return {};
  const record: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) record[key] = item;
  return record;
}

function parseAskInput(input: unknown): AskQuestion[] | null {
  const questions: unknown = asRecord(input).questions;
  if (!Array.isArray(questions)) return null;
  return questions.map((q: unknown) => {
    const record = asRecord(q);
    const options: unknown[] = Array.isArray(record.options) ? record.options : [];
    return {
      question: typeof record.question === 'string' ? record.question : '',
      options: options.map((option: unknown) => {
        const item = asRecord(option);
        return {
          label: typeof item.label === 'string' ? item.label : '',
          description: typeof item.description === 'string' ? item.description : null,
        };
      }),
    };
  });
}

/** AskUserQuestion ToolUse 静态卡：问题与选项原样可读（答案经 composer + resume） */
function AskUserQuestionCard({ block }: { block: ToolUseBlock }): React.JSX.Element {
  const questions = parseAskInput(block.input);
  return (
    <div
      className="my-1 rounded-md border border-border bg-muted px-3 py-2.5"
      data-testid="ask-question-card"
    >
      <div className="mb-1 text-xs font-medium text-muted-foreground">
        向用户提问（AskUserQuestion）· 请在下方输入框作答
      </div>
      {questions === null ? (
        <pre className="m-0 overflow-x-auto rounded bg-background px-2 py-1.5 text-xs">
          {JSON.stringify(block.input, null, 2)}
        </pre>
      ) : (
        questions.map((item, index) => (
          <div key={index} className="mb-2 last:mb-0">
            <div className="text-sm" data-testid="ask-question">
              {item.question}
            </div>
            <ul className="m-0 mt-1 list-none p-0">
              {item.options.map((option, optionIndex) => (
                <li
                  key={optionIndex}
                  className="rounded border border-border bg-background px-2 py-1 text-sm"
                  data-testid="ask-option"
                >
                  {option.label}
                  {option.description !== null && (
                    <span className="ml-1.5 text-xs text-muted-foreground">
                      {option.description}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        ))
      )}
    </div>
  );
}

/** 工具对卡片：ToolUse 与同 id ToolResult 成对呈现（默认折叠，input/result 展开） */
function ToolPairCard({
  block,
  result,
}: {
  block: ToolUseBlock;
  result: ToolResultBlock | null;
}): React.JSX.Element {
  return (
    <details className="my-1" data-testid="block-tool-use" data-tool-id={block.id}>
      <summary className="cursor-pointer text-sm">
        工具调用 <code>{block.name}</code>
        {result !== null && result.isError && (
          <span className="ml-1.5 text-xs text-fail">出错</span>
        )}
      </summary>
      <pre className="mt-1 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
        {JSON.stringify(block.input, null, 2)}
      </pre>
      {result !== null && (
        <div
          className={`mt-1 whitespace-pre-wrap break-words rounded px-2 py-1.5 text-sm ${result.isError ? 'bg-fail-bg text-fail' : 'bg-muted'}`}
          data-testid="block-tool-result"
        >
          {result.content}
        </div>
      )}
    </details>
  );
}

/** 思考块：默认折叠 */
function ThinkingBlock({ thinking }: { thinking: string }): React.JSX.Element {
  return (
    <details className="my-1" data-testid="block-thinking">
      <summary className="cursor-pointer text-xs text-muted-foreground">思考</summary>
      <div className="mt-1 whitespace-pre-wrap break-words border-l-2 border-border pl-2.5 text-sm text-muted-foreground">
        {thinking}
      </div>
    </details>
  );
}

/** 文本块：markdown 渲染（GFM） */
function TextBlock({ text }: { text: string }): React.JSX.Element {
  return (
    <div
      className="prose prose-sm max-w-none break-words prose-pre:rounded prose-pre:bg-background prose-code:bg-background prose-code:text-foreground"
      data-testid="block-text"
    >
      <Markdown remarkPlugins={[remarkGfm]}>{text}</Markdown>
    </div>
  );
}

/** run 结束汇总卡：轮数 / 成本 / 时长 / session（可读） */
function ResultCard({ event }: { event: RunResultEvent }): React.JSX.Element {
  return (
    <div
      className={`my-2 rounded-md border px-3 py-2.5 text-sm ${event.isError ? 'border-fail bg-fail-bg text-fail' : 'border-border bg-muted'}`}
      data-testid="event-result"
      data-is-error={event.isError}
    >
      <div className="mb-1 font-semibold">
        run 结束（{event.subtype}）{event.isError ? '· 失败' : '· 成功'}
      </div>
      <div className="flex flex-wrap gap-x-5 gap-y-1 text-xs" data-testid="result-summary">
        <span>轮数：{event.numTurns ?? '—'}</span>
        <span>成本：{event.costUsd !== null ? `$${event.costUsd}` : '—'}</span>
        <span>时长：{event.durationMs !== null ? `${event.durationMs}ms` : '—'}</span>
        <span>session：{event.sessionId ?? '—'}</span>
      </div>
    </div>
  );
}

/** 气泡内块渲染：AskUserQuestion 静态卡 / 工具对 / 折叠思考 / markdown 文本 */
function ChatBlocks({
  blocks,
  paired,
}: {
  blocks: AgentBlock[];
  paired: Map<string, ToolResultBlock>;
}): React.JSX.Element {
  return (
    <>
      {blocks.map((block, index) => {
        if (block.kind === 'text') return <TextBlock key={index} text={block.text} />;
        if (block.kind === 'thinking')
          return <ThinkingBlock key={index} thinking={block.thinking} />;
        if (block.kind === 'toolUse') {
          if (block.name === 'AskUserQuestion') {
            return <AskUserQuestionCard key={index} block={block} />;
          }
          return <ToolPairCard key={index} block={block} result={paired.get(block.id) ?? null} />;
        }
        // 已与同 id ToolUse 成对的结果不单独重复呈现
        if (paired.has(block.id)) return null;
        return (
          <ToolPairCard
            key={index}
            block={{ kind: 'toolUse', id: block.id, name: '工具调用', input: null }}
            result={block}
          />
        );
      })}
    </>
  );
}

/** 气泡：user / assistant 各成泡（user 靠右），内容块按四变体映射渲染 */
function ChatBubble({
  event,
  paired,
}: {
  event: MessageEvent;
  paired: Map<string, ToolResultBlock>;
}): React.JSX.Element {
  const isUser = event.role !== 'assistant';
  return (
    <Message align={isUser ? 'end' : 'start'} data-testid="chat-bubble" data-role={event.role}>
      <MessageContent
        className={`rounded-lg border border-border px-3 py-2 ${isUser ? 'bg-muted' : 'bg-card'}`}
      >
        <div className="mb-0.5 text-xs text-muted-foreground">{event.role}</div>
        <ChatBlocks blocks={event.blocks} paired={paired} />
      </MessageContent>
    </Message>
  );
}

/** 条目渲染分发（气泡 / 辅助行 / 汇总卡 / raw 透传） */
function EntryView({
  entry,
  paired,
}: {
  entry: Entry;
  paired: Map<string, ToolResultBlock>;
}): React.JSX.Element {
  if (entry.kind === 'chat') return <ChatBubble event={entry.event} paired={paired} />;
  if (entry.kind === 'result') return <ResultCard event={entry.event} />;
  if (entry.kind === 'raw')
    return (
      <div className="py-1.5" data-testid="event-raw">
        <div className="text-xs text-muted-foreground">未识别事件（{entry.eventType}）</div>
        <pre className="mt-0.5 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
          {entry.rawJson}
        </pre>
      </div>
    );
  return (
    <div className="py-1 text-xs text-muted-foreground" data-testid="event-info">
      {entry.text}
    </div>
  );
}

/** 滚动承载：message-scroller 组合（Provider/Root/Viewport/Content/Item + 到底按钮） */
function ConversationScroller({
  entries,
  paired,
}: {
  entries: Entry[];
  paired: Map<string, ToolResultBlock>;
}): React.JSX.Element {
  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end">
      <MessageScroller className="flex-1">
        <MessageScrollerViewport aria-label="对话消息">
          <MessageScrollerContent className="px-3 py-3">
            {entries.length === 0 && (
              <div className="text-muted-foreground" data-testid="conversation-empty">
                尚无对话。在下方输入以开始探索。
              </div>
            )}
            {entries.map((entry) => (
              <MessageScrollerItem key={entry.seq} messageId={String(entry.seq)}>
                <EntryView entry={entry} paired={paired} />
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
 * 对话区：shadcn message-scroller 承载滚动（turn 锚定 + 流式跟随 + 到底按钮，
 * 滚动行为由组件负责，消息状态留在应用层）；事件 → 气泡映射——user /
 * assistant 各成气泡，块四变体（Text→markdown、Thinking→折叠、ToolUse→卡片、
 * ToolResult 与同 id ToolUse 成对），AskUserQuestion 呈静态可读卡，result
 * 汇总（轮数 / 成本 / 时长 / session）可读。
 */
export function ExploreConversation({
  events,
  loading,
  running,
}: ExploreConversationProps): React.JSX.Element {
  const entries = toEntries(events);
  const paired = collectResults(events);
  return (
    <section
      className="flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-card"
      data-testid="explore-conversation"
    >
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <h2 className="m-0 text-[15px]">对话</h2>
        {running && (
          <span className="text-xs text-muted-foreground" data-testid="conversation-running">
            运行中…
          </span>
        )}
      </div>
      {loading && entries.length === 0 ? (
        <div className="p-3 text-muted-foreground" data-testid="conversation-loading">
          会话还原中…
        </div>
      ) : (
        <ConversationScroller entries={entries} paired={paired} />
      )}
    </section>
  );
}
