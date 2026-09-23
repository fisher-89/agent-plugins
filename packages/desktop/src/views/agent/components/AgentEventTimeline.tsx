import type { AgentBlock, AgentEvent } from '../../../types/dto';

export interface AgentEventTimelineProps {
  /** 事件序列（实时累积流或落库重放流，两者共用本组件） */
  events: AgentEvent[];
  /** 运行中标记：头部呈现流式状态 */
  running: boolean;
}

/** 按 parentToolUseId 把子代理消息挂到父 tool_use 调用下（归因分组） */
function groupByParent(events: AgentEvent[]): {
  topLevel: AgentEvent[];
  byParent: Map<string, AgentEvent[]>;
} {
  const topLevel: AgentEvent[] = [];
  const byParent = new Map<string, AgentEvent[]>();
  for (const event of events) {
    const parentId = event.kind === 'message' ? event.parentToolUseId : null;
    if (parentId !== null && parentId !== '') {
      const bucket = byParent.get(parentId);
      if (bucket) bucket.push(event);
      else byParent.set(parentId, [event]);
    } else {
      topLevel.push(event);
    }
  }
  return { topLevel, byParent };
}

/** 可复制值：原文 + 复制按钮（result 汇总卡 sessionId 等一处语义） */
function CopyValue({ value }: { value: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <code className="break-all rounded bg-muted px-1 py-0.5 text-xs">{value}</code>
      <button
        type="button"
        className="cursor-pointer border-0 bg-transparent p-0 text-xs text-muted-foreground underline hover:text-foreground"
        data-testid="copy-value"
        onClick={() => {
          void navigator.clipboard.writeText(value);
        }}
      >
        复制
      </button>
    </span>
  );
}

/** 思考块：默认折叠 */
function ThinkingBlock({ thinking }: { thinking: string }) {
  return (
    <details className="my-1" data-testid="block-thinking">
      <summary className="cursor-pointer text-xs text-muted-foreground">思考</summary>
      <div className="mt-1 whitespace-pre-wrap break-words border-l-2 border-border pl-2.5 text-sm text-muted-foreground">
        {thinking}
      </div>
    </details>
  );
}

/** 工具调用块：默认折叠，input JSON 展开；子代理消息嵌在其下归因呈现 */
function ToolUseBlock({
  block,
  byParent,
}: {
  block: Extract<AgentBlock, { kind: 'toolUse' }>;
  byParent: Map<string, AgentEvent[]>;
}) {
  const children = byParent.get(block.id) ?? [];
  return (
    <details className="my-1" data-testid="block-tool-use" data-tool-id={block.id}>
      <summary className="cursor-pointer text-sm">
        工具调用 <code>{block.name}</code>
      </summary>
      <pre className="mt-1 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
        {JSON.stringify(block.input, null, 2)}
      </pre>
      {children.length > 0 && (
        <div
          className="mt-1.5 border-l-2 border-border pl-2.5"
          data-testid="subagent-group"
          data-parent-id={block.id}
        >
          <div className="mb-1 text-xs text-muted-foreground">子代理（父工具调用 {block.id}）</div>
          {children.map((event) => (
            <TimelineEvent key={event.seq} event={event} byParent={byParent} nested />
          ))}
        </div>
      )}
    </details>
  );
}

/** 工具结果块：默认折叠，content 文本展开；is_error 高亮 */
function ToolResultBlock({ block }: { block: Extract<AgentBlock, { kind: 'toolResult' }> }) {
  return (
    <details
      className="my-1"
      data-testid="block-tool-result"
      data-tool-id={block.id}
      data-is-error={block.isError}
    >
      <summary className="cursor-pointer text-sm">
        工具结果 <span className="text-xs text-muted-foreground">id {block.id}</span>
        {block.isError && <span className="ml-1.5 text-xs text-fail">出错</span>}
      </summary>
      <div
        className={`mt-1 whitespace-pre-wrap break-words rounded px-2 py-1.5 text-sm ${block.isError ? 'bg-fail-bg text-fail' : 'bg-muted'}`}
      >
        {block.content}
      </div>
    </details>
  );
}

function MessageBody({
  event,
  byParent,
}: {
  event: Extract<AgentEvent, { kind: 'message' }>;
  byParent: Map<string, AgentEvent[]>;
}) {
  return (
    <>
      {event.blocks.map((block, index) => {
        const key = `${event.seq}-${index}`;
        switch (block.kind) {
          case 'text':
            return (
              <div
                key={key}
                className="my-1 whitespace-pre-wrap break-words text-sm"
                data-testid="block-text"
              >
                {block.text}
              </div>
            );
          case 'thinking':
            return <ThinkingBlock key={key} thinking={block.thinking} />;
          case 'toolUse':
            return <ToolUseBlock key={key} block={block} byParent={byParent} />;
          case 'toolResult':
            return <ToolResultBlock key={key} block={block} />;
        }
      })}
    </>
  );
}

function MessageEvent({
  event,
  byParent,
  nested,
}: {
  event: Extract<AgentEvent, { kind: 'message' }>;
  byParent: Map<string, AgentEvent[]>;
  nested: boolean;
}) {
  return (
    <div
      className={`border-b border-border py-2 last:border-b-0 ${nested ? 'pl-1.5' : ''}`}
      data-testid="event-message"
      data-role={event.role}
      data-parent-tool-use-id={event.parentToolUseId ?? undefined}
    >
      <div className="mb-0.5 text-xs text-muted-foreground">
        {event.role}
        {event.parentToolUseId !== null && event.parentToolUseId !== '' && (
          <span className="ml-1.5">子代理</span>
        )}
      </div>
      <MessageBody event={event} byParent={byParent} />
    </div>
  );
}

/** runResult 汇总卡：轮数 / 成本 / 时长 / session（可复制）一等公民位 */
function RunResultCard({ event }: { event: Extract<AgentEvent, { kind: 'runResult' }> }) {
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
        <span>
          轮数：<strong data-testid="result-num-turns">{event.numTurns ?? '—'}</strong>
        </span>
        <span>
          成本：
          <strong data-testid="result-cost">
            {event.costUsd !== null ? `$${event.costUsd}` : '—'}
          </strong>
        </span>
        <span>
          时长：
          <strong data-testid="result-duration">
            {event.durationMs !== null ? `${event.durationMs}ms` : '—'}
          </strong>
        </span>
        <span className="flex items-center gap-1">
          session：
          {event.sessionId !== null ? (
            <CopyValue value={event.sessionId} />
          ) : (
            <strong data-testid="result-session">—</strong>
          )}
        </span>
      </div>
    </div>
  );
}

/** 非消息辅助事件：run 启动信息行 / system 通知行 / result 汇总卡 / raw 原文透传 */
function AuxEvent({ event }: { event: Exclude<AgentEvent, { kind: 'message' }> }) {
  if (event.kind === 'runStarted') {
    return (
      <div className="py-2 text-xs text-muted-foreground" data-testid="event-run-started">
        run 启动
        {event.model !== null && <span className="ml-2">模型 {event.model}</span>}
        {event.sessionId !== null && <span className="ml-2">session {event.sessionId}</span>}
        <span className="ml-2">工具 {event.tools.length} 个</span>
        <span className="ml-2">MCP {event.mcpServers.length} 个</span>
      </div>
    );
  }
  if (event.kind === 'systemNotice') {
    return (
      <div className="py-1.5 text-xs text-muted-foreground" data-testid="event-system">
        system 通知（{event.subtype}）
      </div>
    );
  }
  if (event.kind === 'runResult') {
    return <RunResultCard event={event} />;
  }
  // raw：未知 / 非 JSON 行透传原文（保真呈现，不解析）
  return (
    <div className="py-1.5" data-testid="event-raw">
      <div className="text-xs text-muted-foreground">未识别事件（{event.eventType}）</div>
      <pre className="mt-0.5 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs">
        {event.rawJson}
      </pre>
    </div>
  );
}

/** 单事件渲染分发（runStarted / message / systemNotice / runResult / raw） */
function TimelineEvent({
  event,
  byParent,
  nested = false,
}: {
  event: AgentEvent;
  byParent: Map<string, AgentEvent[]>;
  nested?: boolean;
}) {
  if (event.kind === 'message') {
    return <MessageEvent event={event} byParent={byParent} nested={nested} />;
  }
  return <AuxEvent event={event} />;
}

/**
 * 事件时间线（实时流与历史重放共用）：assistant / user 消息渲染为对话流，
 * tool_use / tool_result 折叠块成对呈现（同 id 可对读），子代理消息按
 * parentToolUseId 分组归因到父工具调用下，runResult 汇总卡呈现轮数 / 成本 /
 * 时长 / session（可复制）——loop 调试关键信息在一等公民位。
 */
export function AgentEventTimeline({
  events,
  running,
}: AgentEventTimelineProps): React.JSX.Element {
  const { topLevel, byParent } = groupByParent(events);
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
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
      {events.length === 0 ? (
        <div className="text-muted-foreground" data-testid="timeline-empty">
          尚无事件。
        </div>
      ) : (
        topLevel.map((event) => <TimelineEvent key={event.seq} event={event} byParent={byParent} />)
      )}
    </section>
  );
}
