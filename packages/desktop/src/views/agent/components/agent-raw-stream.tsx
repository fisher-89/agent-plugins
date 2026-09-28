import type { AgentEvent } from '../../../types/dto';

export interface AgentRawStreamProps {
  /** 事件序列（实时累积流或落库重放流） */
  events: AgentEvent[];
}

/**
 * 原始 JSONL 面板：逐事件 JSON dump（落库归一化形态，与重放同源一致）；
 * Raw 变体内嵌 rawJson 原文保真，不为非 Raw 事件另存 CLI 原始行。
 */
export function AgentRawStream({ events }: AgentRawStreamProps): React.JSX.Element {
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-raw-stream"
    >
      <h2 className="m-0 mb-1.5 text-[15px]">原始 JSONL</h2>
      {events.length === 0 ? (
        <div className="text-muted-foreground" data-testid="raw-stream-empty">
          尚无事件。
        </div>
      ) : (
        events.map((event) => (
          <pre
            key={event.seq}
            className="mb-1.5 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs"
            data-testid="raw-line"
            data-seq={event.seq}
          >
            {JSON.stringify(event, null, 2)}
          </pre>
        ))
      )}
    </section>
  );
}
