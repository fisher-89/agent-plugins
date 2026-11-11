import { openUrl } from '@tauri-apps/plugin-opener';
import { CheckCircleIcon, DotIcon, GlobeIcon, ToolboxIcon } from 'lucide-react';
import { memo, useMemo } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { CodeBlock, CodeBlockContent, CodeBlockHeader } from '@/components/ui/code-block';
import { Spinner } from '@/components/ui/spinner';
import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from '@/components/ui/trace';
import type { AgentToolOutput, AgentToolPart, AgentUIMessage } from '@/lib/agent-adapter';
import { asRecord, textOf } from '@/lib/parse';
import type { AgentCitation } from '@/types/dto';

/** 模块级稳定引用：react-markdown 每次渲染都重建 processor，插件数组不必每拍新建 */
const REMARK_PLUGINS = [remarkGfm];

/** 消息部件 → tool 部件收窄（`tool-<name>` 命名，dynamic-tool 不在信封内） */
export function isToolPart(part: AgentUIMessage['parts'][number]): part is AgentToolPart {
  return part.type.startsWith('tool-') && part.type !== 'dynamic-tool';
}

/** 仅 `http:` / `https:` 目标视为可打开链接（其余 scheme 一律降级纯文本） */
function isHttpUrl(url: string | null | undefined): url is string {
  return typeof url === 'string' && /^https?:\/\//i.test(url);
}

/**
 * 外链：**唯一**外链调用点（搜索来源与引用列表共用）。
 *
 * 仅 `http:` / `https:` 目标渲染为可点击 `<a>` 并经壳侧 opener 打开；其他
 * scheme（`javascript:` / `file:` / `mailto:` 等）降级纯文本、不触达插件。
 * 打开失败（scope 拒绝 / 无默认浏览器）以 `.catch` 吞掉——不打断呈现、不改用
 * 其他打开通道。
 */
function ExternalLink({
  url,
  children,
}: {
  url?: string | null;
  children: React.ReactNode;
}): React.JSX.Element {
  if (!isHttpUrl(url)) return <>{children}</>;
  return (
    <a
      className="text-primary underline-offset-2 hover:underline"
      href={url}
      rel="noreferrer"
      target="_blank"
      onClick={(event) => {
        event.preventDefault();
        void openUrl(url).catch(() => {});
      }}
    >
      {children}
    </a>
  );
}

/** 按 url 去重（无 url 条目逐条保留） */
function dedupeByUrl<T extends { url?: string | null }>(items: T[]): T[] {
  const seen = new Set<string>();
  const unique: T[] = [];
  for (const item of items) {
    const url = item.url ?? null;
    if (url !== null) {
      if (seen.has(url)) continue;
      seen.add(url);
    }
    unique.push(item);
  }
  return unique;
}

/**
 * 引用列表：正文 text 部件**底部**的去重列表（`title ?? url ?? citedText`）。
 * MUST NOT 做行内角标、MUST NOT 做原句高亮（raw citation 不保证可定位到渲染
 * 片段，错位即误导）；`citations` 缺席 / 空数组时容器不渲染（调用点已折叠）。
 */
function CitationList({ citations }: { citations: AgentCitation[] }): React.JSX.Element {
  const entries = dedupeByUrl(citations);
  return (
    <ul
      className="m-0 mt-2 flex list-none flex-col gap-0.5 border-t border-border p-0 pt-1.5 text-xs text-muted-foreground"
      data-testid="block-citations"
    >
      {entries.map((citation, index) => (
        <li key={index} className="truncate">
          <ExternalLink url={citation.url}>
            {citation.title ?? citation.url ?? citation.citedText ?? ''}
          </ExternalLink>
        </li>
      ))}
    </ul>
  );
}

/** citations 按渲染字段比较：`PartView` 每拍都重建数组/对象，默认浅比较必然
 * 击穿 memo；`raw` 不参与呈现（`CitationList` 只用 url/title/citedText），不进
 * 比较。未来若在卡面渲染新字段，必须同步加入此处。 */
function sameCitations(left?: AgentCitation[], right?: AgentCitation[]): boolean {
  if (left === right) return true;
  const a = left ?? [];
  const b = right ?? [];
  if (a.length !== b.length) return false;
  return a.every((item, index) => {
    const other = b[index];
    return (
      item.url === other.url && item.title === other.title && item.citedText === other.citedText
    );
  });
}

/** 文本块：markdown 渲染（GFM）+ 非空 citations 的底部引用列表 */
function TextBlockBase({
  text,
  citations,
}: {
  text: string;
  citations?: AgentCitation[];
}): React.JSX.Element {
  // 文本未变即复用同一 React 元素：React 的同元素引用 bailout 会跳过
  // react-markdown 的 processor 重建与全量 parse（流式期间只解析变化中的文本）。
  const markdown = useMemo(
    () => <Markdown remarkPlugins={REMARK_PLUGINS}>{text}</Markdown>,
    [text],
  );
  return (
    <div
      className="prose prose-sm dark:prose-invert [&_pre]:text-[size:inherit] max-w-full"
      data-testid="block-text"
    >
      {markdown}
      {citations !== undefined && citations.length > 0 && <CitationList citations={citations} />}
    </div>
  );
}

export const TextBlock = memo(
  TextBlockBase,
  (prev, next) => prev.text === next.text && sameCitations(prev.citations, next.citations),
);

/** 思考块：默认折叠 */
function ThinkingBlockBase({
  text,
  state,
}: {
  text: string;
  state?: 'streaming' | 'done';
}): React.JSX.Element {
  return (
    <TraceCollapsible data-testid="block-thinking">
      <TraceCollapsibleTrigger>
        <TraceLine status={state === 'streaming' ? 'pending' : undefined}>
          <TraceIcon>{state === 'streaming' ? <Spinner /> : <DotIcon />}</TraceIcon>
          <TraceTitle>{state === 'streaming' ? '思考中…' : '思考'}</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>{text}</TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const ThinkingBlock = memo(ThinkingBlockBase);

/** 工具对卡片：input 与同 id output 成对呈现（默认折叠，isError 高亮） */
function ToolPairCardBase({
  name,
  toolCallId,
  state,
  input,
  output,
}: {
  name: string;
  toolCallId: string;
  state: 'input-available' | 'output-available';
  input: unknown;
  output: AgentToolOutput | null;
}): React.JSX.Element {
  return (
    <TraceCollapsible data-testid="block-tool-use" data-tool-id={toolCallId}>
      <TraceCollapsibleTrigger>
        <TraceLine status={output?.isError ? 'error' : undefined}>
          <TraceIcon>{state === 'output-available' ? <ToolboxIcon /> : <Spinner />}</TraceIcon>
          <TraceTitle>{name}</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        <CodeBlock>
          <CodeBlockHeader>输入</CodeBlockHeader>
          <CodeBlockContent className="max-h-48">{JSON.stringify(input, null, 2)}</CodeBlockContent>
          <CodeBlockHeader>输出</CodeBlockHeader>
          {output !== null && (
            <CodeBlockContent className="max-h-48" data-testid="block-tool-result">
              {output.content}
            </CodeBlockContent>
          )}
        </CodeBlock>
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const ToolPairCard = memo(ToolPairCardBase);

/** 查询词：`input.query`（`input` 为 `null` / 非对象 / 无字符串 query → `null`，
 * 该行省略） */
function searchQuery(input: unknown): string | null {
  const query = textOf(asRecord(input).query);
  return query === null || query === '' ? null : query;
}

/**
 * 云端 `web_search` 搜索卡（复用工具卡管线承载，`DEFAULT_TOOL_CARDS` 注册项）。
 *
 * summary **恒为「网络搜索」**——无「正在搜索…」态、无超时降级文案、不依赖流内
 * 状态切换（决策 ③：本变更无搜索实时 delta，前端没有任何信号可驱动进行态）；
 * 「未落定 / 已落定」仅由 `output` 在场与否决定，两态视觉一致。body 三区：
 * 查询词 / 来源列表（`title ?? url`，按 url 去重）/ 错误码（非空时）。
 */
function WebSearchCardBase({
  toolCallId,
  input,
  output,
}: {
  toolCallId: string;
  toolName: string;
  input: unknown;
  output: AgentToolOutput | null;
}): React.JSX.Element {
  const query = searchQuery(input);
  const content = output?.content ?? '';
  const search = output?.search ?? null;
  const sources = search === null ? [] : dedupeByUrl(search.results);
  const error = search === null || search.error === '' ? null : search.error;
  return (
    <TraceCollapsible data-testid="block-web-search" data-tool-id={toolCallId}>
      <TraceCollapsibleTrigger>
        <TraceLine status={error ? 'error' : undefined}>
          <TraceIcon>
            <GlobeIcon />
          </TraceIcon>
          <TraceTitle data-testid={query === null ? undefined : 'block-web-search-query'}>
            {query}
          </TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        {error !== null ? (
          <div className="text-xs text-fail" data-testid="block-web-search-error">
            {error}
          </div>
        ) : (
          <div>
            {content}
            {sources.length > 0 && (
              <Trace data-testid="block-web-search-sources">
                {sources.map((source) => (
                  <TraceLine>
                    <TraceIcon>
                      <GlobeIcon />
                    </TraceIcon>
                    <TraceTitle>
                      <ExternalLink url={source.url}>
                        {source.title ?? new URL(source.url).hostname}
                      </ExternalLink>
                    </TraceTitle>
                  </TraceLine>
                ))}
              </Trace>
            )}
          </div>
        )}
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const WebSearchCard = memo(WebSearchCardBase);

/** `finish_step` 收口卡的逐项清单行（evaluator 形态）。 */
interface FinishItem {
  id: string;
  pass: boolean;
  evidence: string | null;
}

/** `items` 宽窄解析：非数组即 `null`（回退 JSON 原样展示）。 */
function parseFinishItems(value: unknown): FinishItem[] | null {
  if (!Array.isArray(value)) return null;
  return value.map((item: unknown) => {
    const record = asRecord(item);
    return {
      id: textOf(record.id) ?? '',
      pass: record.pass === true,
      evidence: textOf(record.evidence),
    };
  });
}

/** 清单摘要行：全通过 / 含未通过项（纯展示推导，不写任何持久面）。 */
function finishSummary(items: FinishItem[]): string {
  return items.every((item) => item.pass) ? '清单全通过' : '含未通过项';
}

/**
 * decision 形态的动作摘要行（`action` 为字符串时命中）：
 * `backtrack` 呈目标与理由、`stop` 呈理由、`ask` 呈问题与选项，未知动作原样。
 */
function decisionSummary(record: Record<string, unknown>): string {
  const action = textOf(record.action) ?? '';
  const reason = textOf(record.reason) ?? '';
  switch (action) {
    case 'backtrack':
      return `backtrack → ${textOf(record.to) ?? ''}：${reason}`;
    case 'stop':
      return `stop：${reason}`;
    case 'ask': {
      const options = Array.isArray(record.options)
        ? record.options.map((option) => textOf(option) ?? '').join(' / ')
        : '';
      return `ask：${textOf(record.question) ?? ''}${options === '' ? '' : `（${options}）`}`;
    }
    default:
      return action;
  }
}

/**
 * `finish_step` 收口卡（复用工具卡管线的注册表项，`BUILD_IN_TOOL_CARDS`）：
 * evaluator 形态（`input.items` 为数组）呈逐项清单 + report；decision 形态
 * （`input.action` 为字符串）呈动作摘要行；两形态均不匹配时回退 JSON 原样
 * （沿 `AskUserQuestionCard` 的宽容解析先例，不抛错）。
 */
function FinishStepCardBase({
  toolCallId,
  input,
  output,
}: {
  toolCallId: string;
  toolName: string;
  input: unknown;
  output: AgentToolOutput | null;
}): React.JSX.Element {
  const record = asRecord(input);
  const items = parseFinishItems(record.items);
  const action = textOf(record.action);
  const summary =
    items !== null ? finishSummary(items) : action !== null ? decisionSummary(record) : null;
  return (
    <TraceCollapsible data-testid="block-tool-use" data-tool-id={toolCallId}>
      <TraceCollapsibleTrigger>
        <TraceLine status={output?.isError ? 'error' : undefined}>
          <TraceIcon>
            <CheckCircleIcon />
          </TraceIcon>
          <TraceTitle data-testid="block-finish-step-summary">
            {summary ?? 'finish_step'}
          </TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        <div className="flex flex-col gap-1 text-xs">
          {output !== null && (
            <div className="text-muted-foreground" data-testid="block-tool-result">
              {output.content}
            </div>
          )}
          {items !== null ? (
            <>
              <ul className="m-0 list-none p-0">
                {items.map((item, index) => (
                  <li
                    key={index}
                    className="flex gap-1.5 truncate"
                    data-testid="block-finish-step-item"
                  >
                    <span className="font-medium">{item.id}</span>
                    <span className={item.pass ? 'text-pass' : 'text-fail'}>
                      {item.pass ? '通过' : '未通过'}
                    </span>
                    {item.evidence !== null && (
                      <span className="truncate text-muted-foreground">{item.evidence}</span>
                    )}
                  </li>
                ))}
              </ul>
              {textOf(record.report) !== null && (
                <div className="whitespace-pre-wrap text-muted-foreground">
                  {textOf(record.report)}
                </div>
              )}
            </>
          ) : action === null ? (
            <pre className="m-0 overflow-x-auto rounded bg-background px-2 py-1.5">
              {JSON.stringify(input, null, 2)}
            </pre>
          ) : null}
        </div>
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const FinishStepCard = memo(FinishStepCardBase);
