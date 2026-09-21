import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import type { ArtifactEnvelope } from '../types/dto';

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

/** markdown-doc renderer：payload.markdown 经 react-markdown + GFM 扩展渲染（表格/任务列表等） */
export function MarkdownDocRenderer({ envelope }: { envelope: ArtifactEnvelope }) {
  if (!isMarkdownDocPayload(envelope.payload)) {
    return <pre className="fallback-text">{envelope.fallbackText ?? '（无 markdown 内容）'}</pre>;
  }
  return (
    <div className="markdown-doc">
      <Markdown remarkPlugins={[remarkGfm]}>{envelope.payload.markdown}</Markdown>
    </div>
  );
}
