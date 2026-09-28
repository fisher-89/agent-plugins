import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import type { ArtifactEnvelope } from '../../../types/dto';

/** markdown-doc payload 契约（design 数据模型表）：{ markdown: string } */
interface MarkdownDocPayload {
  markdown: string;
}

/** 类型守卫：以 payload 实际形状收窄，替代 as 断言（payload 契约为 unknown） */
function isMarkdownDocPayload(value: unknown): value is MarkdownDocPayload {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { markdown?: unknown }).markdown === 'string'
  );
}

/** markdown-doc renderer：payload.markdown 经 react-markdown + GFM 扩展渲染（表格/任务列表等）。
 * 裸元素后代样式由 @tailwindcss/typography 的 prose 承载（prose-* modifier 对齐原观感），
 * 不再存在 .markdown-doc 后代选择器（AC-7）。 */
export function MarkdownDocRenderer({ envelope }: { envelope: ArtifactEnvelope }) {
  if (!isMarkdownDocPayload(envelope.payload)) {
    return (
      <pre
        className="m-0 max-h-[320px] overflow-auto whitespace-pre-wrap break-words rounded-md bg-background px-2.5 py-2 text-xs text-foreground"
        data-testid="fallback-text"
      >
        {envelope.fallbackText ?? '（无 markdown 内容）'}
      </pre>
    );
  }
  return (
    <div
      className="prose prose-sm prose-invert max-w-none overflow-x-auto [--tw-prose-links:var(--primary)] [--tw-prose-td-borders:var(--border)] prose-code:rounded-sm prose-code:bg-background prose-code:font-normal prose-code:text-foreground prose-pre:rounded prose-pre:bg-background prose-pre:text-foreground prose-th:bg-background prose-th:font-semibold prose-th:text-foreground"
      data-testid="markdown-root"
    >
      <Markdown remarkPlugins={[remarkGfm]}>{envelope.payload.markdown}</Markdown>
    </div>
  );
}
