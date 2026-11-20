import { cn } from 'cn';
import { ChevronDownIcon } from 'lucide-react';
import { memo, useState } from 'react';

import { Message, MessageContent } from '@/components/ui';
import { Bubble, BubbleContent } from '@/components/ui/bubble';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleTrigger } from '@/components/ui/collapsible';
import { type AgentUIMessage } from '@/lib/agent-adapter';

import { TextBlock } from './blocks';

/** user 提问折叠的可见行上限（显式换行与按字符估算的折行合计） */
const USER_COLLAPSE_MAX_LINES = 3;
/** 折行估算的单行字符数：气泡整宽约容 28 个全角字符，取保守值 */
const USER_COLLAPSE_LINE_CHARS = 40;

/** user 提问的正文：全部部件均为 text 时拼接（非 text 部件 → `null`，不参与折叠） */
function userPartsText(message: AgentUIMessage): string | null {
  const lines: string[] = [];
  for (const part of message.parts) {
    if (part.type !== 'text') return null;
    lines.push(part.text);
  }
  return lines.join('\n');
}

/** 估算可见行数：显式换行逐条计，单行按字符数折算折行 */
function userTextLines(text: string): number {
  return text
    .split('\n')
    .reduce(
      (lines, line) => lines + Math.max(1, Math.ceil(line.length / USER_COLLAPSE_LINE_CHARS)),
      0,
    );
}

/** user 气泡正文：短提问照常渲染部件；超长时默认折叠为纯文本预览，
 * 展开后回到 markdown / 部件渲染 */
function UserBubbleBody({ message }: { message: AgentUIMessage }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const text = userPartsText(message);
  const collapsible = text !== null && userTextLines(text) > USER_COLLAPSE_MAX_LINES;
  if (!collapsible) {
    return <TextBlock text={text ?? ''} />;
  }
  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className={cn('overflow-hidden', { 'h-12 **:m-0.5 mask-b-to-card/15': !open })}>
        <TextBlock text={text} />
      </div>
      <CollapsibleTrigger
        data-testid="user-bubble-toggle"
        render={
          <Button
            variant="link"
            className="h-auto gap-1 p-0 text-muted-foreground hover:text-foreground"
          />
        }
      >
        {open ? '收起' : '展开'}
        <ChevronDownIcon
          data-icon="inline-end"
          className="transition-transform group-data-panel-open/button:rotate-180"
        />
      </CollapsibleTrigger>
    </Collapsible>
  );
}

function UserMessageBase({ message }: { message: AgentUIMessage }): React.JSX.Element {
  return (
    <Message data-testid="chat-bubble" data-role={message.role}>
      <MessageContent>
        <Bubble variant="tinted" className="w-full max-w-full">
          <BubbleContent className="w-full">
            <UserBubbleBody message={message} />
          </BubbleContent>
        </Bubble>
      </MessageContent>
    </Message>
  );
}

export const UserMessage = memo(UserMessageBase);
