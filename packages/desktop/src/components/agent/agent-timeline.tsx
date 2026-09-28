/**
 * 保真透镜（loop 调试取向）：按数组序（seq 保序）呈现、消息部件展开式
 * 渲染（tool input/output 对读）、子代理消息按 metadata.parentToolUseId
 * 归因到父工具卡下（subagent-group）、raw 原文透传、result 汇总可复制。
 * 滚动沿用 shadcn message-scroller 组合（流式跟随 + 到底按钮）——嵌入限高
 * 普通块容器（历史重放区）时无 flex 语境自然高，跟随静默 no-op。
 * 取代 views/agent/components/agent-event-timeline.tsx（既有 agent-timeline /
 * event-* / block-* / subagent-group / result-* testid 沿用）。
 */

import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';

import type { AgentRawData, AgentToolPart, AgentUIMessage } from '../../lib/agent-adapter';
import { ResultCard, RunRecordRow, ThinkingBlock, isToolPart } from './agent-blocks';

export interface AgentTimelineProps {
  /** 消息序列（实时累积流或落库重放流，共用本透镜；数组序即 seq 序） */
  messages: AgentUIMessage[];
  /** 运行中标记：头部呈现流式状态 */
  running: boolean;
}

/** 按 metadata.parentToolUseId 把子代理消息挂到父工具调用下（归因分组） */
function groupByParent(messages: AgentUIMessage[]): {
  topLevel: AgentUIMessage[];
  byParent: Map<string, AgentUIMessage[]>;
} {
  const topLevel: AgentUIMessage[] = [];
  const byParent = new Map<string, AgentUIMessage[]>();
  for (const message of messages) {
    const parentId = message.metadata?.parentToolUseId ?? null;
    if (parentId !== null && parentId !== '') {
      const bucket = byParent.get(parentId);
      if (bucket) bucket.push(message);
      else byParent.set(parentId, [message]);
    } else {
      topLevel.push(message);
    }
  }
  return { topLevel, byParent };
}

/** tool 部件名（`tool-<name>` 命名还原） */
function toolName(part: AgentToolPart): string {
  return part.type.slice('tool-'.length);
}

/** raw 原文透传（保真呈现，不解析） */
function RawView({ data }: { data: AgentRawData }): React.JSX.Element {
  return (
    <div className="py-1.5" data-testid="event-raw">
      <div className="text-xs text-muted-foreground">未识别事件（{data.eventType}）</div>
      <pre className="mt-0.5 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
        {data.rawJson}
      </pre>
    </div>
  );
}

/** 工具部件展开卡：input JSON + output 文本对读，子代理消息嵌其下归因 */
function TimelineToolPart({
  part,
  byParent,
}: {
  part: AgentToolPart;
  byParent: Map<string, AgentUIMessage[]>;
}): React.JSX.Element {
  const name = toolName(part);
  const output = part.state === 'output-available' ? part.output : null;
  const nested = byParent.get(part.toolCallId) ?? [];
  return (
    <details className="my-1" data-testid="block-tool-use" data-tool-id={part.toolCallId}>
      <summary className="cursor-pointer text-sm">
        工具调用 <code>{name}</code>
        {output !== null && output.isError && (
          <span className="ml-1.5 text-xs text-fail">出错</span>
        )}
      </summary>
      <pre className="mt-1 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
        {JSON.stringify(part.input, null, 2)}
      </pre>
      {output !== null && (
        <div
          className={`mt-1 whitespace-pre-wrap break-words rounded px-2 py-1.5 text-sm ${output.isError ? 'bg-fail-bg text-fail' : 'bg-muted'}`}
          data-testid="block-tool-result"
        >
          {output.content}
        </div>
      )}
      {nested.length > 0 && (
        <div
          className="mt-1.5 border-l-2 border-border pl-2.5"
          data-testid="subagent-group"
          data-parent-id={part.toolCallId}
        >
          <div className="mb-1 text-xs text-muted-foreground">
            子代理（父工具调用 {part.toolCallId}）
          </div>
          {nested.map((message) => (
            <TimelineMessage key={message.id} message={message} byParent={byParent} nested />
          ))}
        </div>
      )}
    </details>
  );
}

/** 单消息部件分发（text 平文 / reasoning 折叠 / tool 展开卡 / data 部件） */
function TimelinePart({
  part,
  byParent,
}: {
  part: AgentUIMessage['parts'][number];
  byParent: Map<string, AgentUIMessage[]>;
}): React.JSX.Element | null {
  if (part.type === 'text') {
    return (
      <div className="my-1 whitespace-pre-wrap break-words text-sm" data-testid="block-text">
        {part.text}
      </div>
    );
  }
  if (part.type === 'reasoning') return <ThinkingBlock text={part.text} />;
  if (isToolPart(part)) return <TimelineToolPart part={part} byParent={byParent} />;
  if (part.type === 'data-run-result') return <ResultCard data={part.data} />;
  if (part.type === 'data-run-record') return <RunRecordRow record={part.data} />;
  if (part.type === 'data-raw') return <RawView data={part.data} />;
  if (part.type === 'data-run-started') {
    return (
      <div className="py-2 text-xs text-muted-foreground" data-testid="event-run-started">
        run 启动
        {part.data.model !== null && <span className="ml-2">模型 {part.data.model}</span>}
        {part.data.sessionId !== null && (
          <span className="ml-2">session {part.data.sessionId}</span>
        )}
        <span className="ml-2">工具 {part.data.tools.length} 个</span>
        <span className="ml-2">MCP {part.data.mcpServers.length} 个</span>
      </div>
    );
  }
  if (part.type === 'data-system-notice') {
    return (
      <div className="py-1.5 text-xs text-muted-foreground" data-testid="event-system">
        system 通知（{part.data.subtype}）
      </div>
    );
  }
  return null;
}

/**
 * 单消息渲染（子代理嵌套缩进；role + 归因标注在头）。顶层级无自带分隔——
 * 分隔由 message-scroller Content 的 divide-y 承载（Item 包裹后 last: 不再
 * 语义成立）；嵌套组内保留自带分隔线。
 */
function TimelineMessage({
  message,
  byParent,
  nested = false,
}: {
  message: AgentUIMessage;
  byParent: Map<string, AgentUIMessage[]>;
  nested?: boolean;
}): React.JSX.Element {
  const parentId = message.metadata?.parentToolUseId ?? null;
  return (
    <div
      className={`py-2 ${nested ? 'border-b border-border pl-1.5 last:border-b-0' : ''}`}
      data-testid="event-message"
      data-role={message.role}
      data-parent-tool-use-id={parentId ?? undefined}
    >
      <div className="mb-0.5 text-xs text-muted-foreground">
        {message.role}
        {parentId !== null && parentId !== '' && <span className="ml-1.5">子代理</span>}
      </div>
      {message.parts.map((part, index) => (
        <TimelinePart key={index} part={part} byParent={byParent} />
      ))}
    </div>
  );
}

/**
 * 事件时间线（保真透镜，实时流与历史重放共用）：消息按数组序展开呈现，
 * tool input / output 对读，子代理消息按 parentToolUseId 归因到父工具卡下，
 * runResult 汇总卡（轮数 / 成本 / 时长 / session 可复制）在一等公民位——
 * loop 调试关键信息全量可考。区域滚动经 message-scroller 组合（流式跟随 +
 * 到底按钮）；嵌入限高普通块容器（历史重放区）时自然高、由外层限高容器滚动。
 */
export function AgentTimeline({ messages, running }: AgentTimelineProps): React.JSX.Element {
  const { topLevel, byParent } = groupByParent(messages);
  return (
    <section
      className="mb-4 flex min-h-0 flex-1 flex-col rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-timeline"
    >
      <h2 className="m-0 mb-1.5 flex items-center gap-2 text-[15px]">
        事件时间线
        {running && (
          <span className="text-xs text-muted-foreground" data-testid="timeline-running">
            运行中…
          </span>
        )}
      </h2>
      {messages.length === 0 ? (
        <div className="text-muted-foreground" data-testid="timeline-empty">
          尚无事件。
        </div>
      ) : (
        <MessageScrollerProvider autoScroll defaultScrollPosition="end">
          <MessageScroller className="min-h-0 flex-1">
            <MessageScrollerViewport aria-label="事件时间线">
              <MessageScrollerContent className="gap-0 divide-y divide-border">
                {topLevel.map((message) => (
                  <MessageScrollerItem key={message.id} messageId={message.id}>
                    <TimelineMessage message={message} byParent={byParent} />
                  </MessageScrollerItem>
                ))}
              </MessageScrollerContent>
            </MessageScrollerViewport>
            <MessageScrollerButton direction="end" />
          </MessageScroller>
        </MessageScrollerProvider>
      )}
    </section>
  );
}
