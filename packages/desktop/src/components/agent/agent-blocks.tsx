import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import type { AgentToolOutput, AgentToolPart, AgentUIMessage } from '../../lib/agent-adapter';

/** 消息部件 → tool 部件收窄（`tool-<name>` 命名，dynamic-tool 不在信封内） */
export function isToolPart(part: AgentUIMessage['parts'][number]): part is AgentToolPart {
  return part.type.startsWith('tool-') && part.type !== 'dynamic-tool';
}

/** 文本块：markdown 渲染（GFM） */
export function TextBlock({ text }: { text: string }): React.JSX.Element {
  return (
    <div
      className="prose prose-sm prose-invert max-w-none break-words [--tw-prose-links:var(--primary)] prose-pre:rounded prose-pre:bg-background prose-code:bg-background prose-code:text-foreground"
      data-testid="block-text"
    >
      <Markdown remarkPlugins={[remarkGfm]}>{text}</Markdown>
    </div>
  );
}

/** 思考块：默认折叠 */
export function ThinkingBlock({ text }: { text: string }): React.JSX.Element {
  return (
    <details className="my-1" data-testid="block-thinking">
      <summary className="cursor-pointer text-xs text-muted-foreground">思考</summary>
      <div className="mt-1 whitespace-pre-wrap break-words border-l-2 border-border pl-2.5 text-sm text-muted-foreground">
        {text}
      </div>
    </details>
  );
}

/** 工具对卡片：input 与同 id output 成对呈现（默认折叠，isError 高亮） */
export function ToolPairCard({
  name,
  toolCallId,
  input,
  output,
}: {
  name: string;
  toolCallId: string;
  input: unknown;
  output: AgentToolOutput | null;
}): React.JSX.Element {
  return (
    <details className="my-1" data-testid="block-tool-use" data-tool-id={toolCallId}>
      <summary className="cursor-pointer text-sm">
        工具调用 <code>{name}</code>
        {output !== null && output.isError && (
          <span className="ml-1.5 text-xs text-fail">出错</span>
        )}
      </summary>
      <pre className="mt-1 overflow-x-auto rounded bg-muted px-2 py-1.5 text-xs break-words">
        {JSON.stringify(input, null, 2)}
      </pre>
      {output !== null && (
        <div
          className={`mt-1 whitespace-pre-wrap break-words rounded px-2 py-1.5 text-sm ${output.isError ? 'bg-fail-bg text-fail' : 'bg-muted'}`}
          data-testid="block-tool-result"
        >
          {output.content}
        </div>
      )}
    </details>
  );
}
