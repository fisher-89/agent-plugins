import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from '@/components/ui/message-scroller';
import { type AgentEvent } from '@/types/dto';

type RunResultEvent = Extract<AgentEvent, { kind: 'runResult' }>;
type Entry =
  | { kind: 'chat'; seq: number; event: MessageEvent }
  | { kind: 'info'; seq: number; text: string }
  | { kind: 'result'; seq: number; event: RunResultEvent }
  | { kind: 'raw'; seq: number; eventType: string; rawJson: string };

export function AgentMessages({
  entries,
  // paired,
}: {
  entries: Entry[];
  // paired: Map<string, ToolResultBlock>;
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
                {/* <EntryView entry={entry} paired={paired} /> */}
              </MessageScrollerItem>
            ))}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton direction="end" />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}
