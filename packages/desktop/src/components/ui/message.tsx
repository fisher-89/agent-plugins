import * as React from 'react';

import { cn } from '@/lib/utils';

/* vendored 内部化：registry 原件按本 app 实际用量收敛（knip 纪律）——保留
   Message / MessageContent 两件（explore 对话气泡），MessageGroup / Avatar /
   Header / Footer 未消费删减；registry 上游形态见 shadcn message 件。 */

function Message({
  className,
  align = 'start',
  ...props
}: React.ComponentProps<'div'> & { align?: 'start' | 'end' }) {
  return (
    <div
      data-slot="message"
      data-align={align}
      className={cn(
        'group/message relative flex w-full min-w-0 gap-2 text-sm data-[align=end]:flex-row-reverse',
        className,
      )}
      {...props}
    />
  );
}

function MessageContent({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot="message-content"
      className={cn(
        'flex w-full min-w-0 flex-col gap-2.5 wrap-break-word group-data-[align=end]/message:*:data-slot:self-end',
        className,
      )}
      {...props}
    />
  );
}

export { Message, MessageContent };
