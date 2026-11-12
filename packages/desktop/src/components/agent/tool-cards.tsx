import { FileEditIcon, FileIcon, TerminalIcon } from 'lucide-react';
import type { ComponentType } from 'react';

import {
  CodeBlock,
  CodeBlockContent,
  CodeBlockHeader,
  CodeBlockLine,
} from '@/components/ui/code-block';
import {
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from '@/components/ui/trace';
import { useHighlightedCode } from '@/hooks/use-highlighted-code';
import type { AgentToolOutput } from '@/lib/agent-adapter';
import { asRecord, numberOf, textOf } from '@/lib/parse';

import { Spinner } from '../ui/spinner';
import { FinishStepCard, WebSearchCard } from './agent-blocks';

/** 特化工具卡入参（output 为 null 即尚无结果；渲染 or null 回退由卡自决） */
interface AgentToolCardProps {
  toolCallId: string;
  toolName: string;
  state: 'input-available' | 'output-available';
  input: unknown;
  output: AgentToolOutput | null;
}

/** 特化工具卡可插拔位类型（按组件装配，允许 memo 包装） */
export type AgentToolCard = ComponentType<AgentToolCardProps>;

/** AskUserQuestion 静态卡的问题/选项形态（宽容解析，未知形态回退 JSON 原样） */
interface AskOption {
  label: string;
  description: string | null;
}
interface AskQuestion {
  question: string;
  options: AskOption[];
}

function parseOptions(options: unknown[]): AskOption[] {
  return options.map((option: unknown) => {
    const item = asRecord(option);
    return {
      label: typeof item.label === 'string' ? item.label : '',
      description: typeof item.description === 'string' ? item.description : null,
    };
  });
}

/** 宽松解析 questions 清单（非数组即 null → 卡内回退 JSON 展示） */
function parseAskInput(input: unknown): AskQuestion[] | null {
  const questions: unknown = asRecord(input).questions;
  if (!Array.isArray(questions)) return null;
  return questions.map((q: unknown) => {
    const record = asRecord(q);
    return {
      question: typeof record.question === 'string' ? record.question : '',
      options: Array.isArray(record.options) ? parseOptions(record.options) : [],
    };
  });
}

/** 单问题：题干 + 选项清单（静态可读，不做交互） */
function AskQuestionView({ item }: { item: AskQuestion }): React.JSX.Element {
  return (
    <div className="mb-2 last:mb-0">
      <div className="text-sm" data-testid="ask-question">
        {item.question}
      </div>
      <ul className="m-0 mt-1 list-none p-0">
        {item.options.map((option, optionIndex) => (
          <li
            key={optionIndex}
            className="rounded border border-border bg-background px-2 py-1 text-sm"
            data-testid="ask-option"
          >
            {option.label}
            {option.description !== null && (
              <span className="ml-1.5 text-xs text-muted-foreground">{option.description}</span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** AskUserQuestion 特化卡：问题与选项原样可读（input 不可解析时回退 JSON） */
function AskUserQuestionCard({ input }: AgentToolCardProps): React.JSX.Element {
  const questions = parseAskInput(input);
  return (
    <div
      className="my-1 rounded-md border border-border bg-muted px-3 py-2.5"
      data-testid="ask-question-card"
    >
      <div className="mb-1 text-xs font-medium text-muted-foreground">
        向用户提问（AskUserQuestion）· 请在下方输入框作答
      </div>
      {questions === null ? (
        <pre className="m-0 overflow-x-auto rounded bg-background px-2 py-1.5 text-xs">
          {JSON.stringify(input, null, 2)}
        </pre>
      ) : (
        questions.map((item, index) => <AskQuestionView key={index} item={item} />)
      )}
    </div>
  );
}

/** 特化卡共用的卡根钩子：与默认对卡同形（消费方按 `block-tool-use` 定位一张工具卡） */
function cardAttrs(toolCallId: string): {
  'data-testid': string;
  'data-tool-id': string;
} {
  return { 'data-testid': 'block-tool-use', 'data-tool-id': toolCallId };
}

/** Bash 特化卡：命令作标题栏，输出折叠在下方（命令形态缺席时标题栏留空） */
function BashCard({ toolCallId, input, output }: AgentToolCardProps): React.JSX.Element {
  const command = textOf(asRecord(input).command);
  return (
    <TraceCollapsible {...cardAttrs(toolCallId)} disabled={output === null}>
      <TraceCollapsibleTrigger>
        <TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>
          <TraceIcon>{output === null ? <Spinner /> : <TerminalIcon />}</TraceIcon>
          <TraceTitle>Bash</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        <CodeBlock>
          <CodeBlockHeader>{command}</CodeBlockHeader>
          <CodeBlockContent className="max-h-60" wrap data-testid="block-tool-result">
            {output?.content}
          </CodeBlockContent>
        </CodeBlock>
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

/** 行区间后缀：`offset` 在场时展示 `:起-止`（`limit` 缺席即「最后」） */
function rangeSuffix(offset: unknown, limit: unknown): string {
  if (typeof offset !== 'number') return '';
  const end = typeof limit === 'number' ? `${offset + limit}` : '最后';
  return `:${offset}-${end}`;
}

/** Read 特化卡：路径 + 行区间作标题，内容折叠在下方 */
function ReadCard({ toolCallId, input, output }: AgentToolCardProps): React.JSX.Element {
  const record = asRecord(input);
  const path = textOf(record.path);
  const offset = numberOf(record.offset);
  return (
    <TraceCollapsible {...cardAttrs(toolCallId)} disabled={output === null}>
      <TraceCollapsibleTrigger>
        <TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>
          <TraceIcon>
            <FileIcon />
          </TraceIcon>
          <TraceTitle>Read {`${path}${rangeSuffix(record.offset, record.limit)}`}</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        {output?.isError && <div className="text-destructive mb-2">{output.content}</div>}
        <CodeBlock>
          <CodeBlockHeader>{path}</CodeBlockHeader>
          <CodeBlockContent
            className="max-h-60"
            data-testid="block-tool-result"
            lineNumbers
            startLine={offset ?? 1}
          >
            {output?.content}
          </CodeBlockContent>
        </CodeBlock>
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

/** 语法名：路径扩展名（无扩展名 → 无高亮语言） */
function languageOf(path: string | null): string | null {
  if (path === null) return null;
  const suffix = path.split('.').at(-1);
  return suffix === undefined || suffix === path ? null : suffix;
}

/** Write 特化卡：写入内容按路径扩展名着色，无扩展名按纯文本切行 */
function WriteCard({ toolCallId, input, output }: AgentToolCardProps): React.JSX.Element {
  const record = asRecord(input);
  const path = textOf(record.path);
  const content = textOf(record.content) ?? '';
  const language = languageOf(path);
  const highlighted = useHighlightedCode(content, language ?? '');
  const lines = language === null ? content.split(/\n/) : highlighted;
  return (
    <TraceCollapsible
      {...cardAttrs(toolCallId)}
      disabled={output === null}
      defaultOpen={output !== null}
    >
      <TraceCollapsibleTrigger>
        <TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>
          <TraceIcon>
            <FileEditIcon />
          </TraceIcon>
          <TraceTitle>Write {path}</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        {output?.isError ? (
          <div className="text-destructive mb-2">{output.content}</div>
        ) : (
          <CodeBlock>
            <CodeBlockHeader>{path}</CodeBlockHeader>
            <CodeBlockContent className="max-h-60" lineNumbers>
              {lines.map((line, index) => (
                <CodeBlockLine key={index}>{line}</CodeBlockLine>
              ))}
            </CodeBlockContent>
          </CodeBlock>
        )}
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

function EditCard({ toolCallId, input, output }: AgentToolCardProps): React.JSX.Element {
  const record = asRecord(input);
  const path = textOf(record.path);
  const language = languageOf(path);
  const oldContent = textOf(record.old_string) ?? '';
  const oldHighlighted = useHighlightedCode(oldContent, language ?? '');
  const oldLines = language === null ? oldContent.split(/\n/) : oldHighlighted;
  const newContent = textOf(record.new_string) ?? '';
  const newHighlighted = useHighlightedCode(newContent, language ?? '');
  const newLines = language === null ? newContent.split(/\n/) : newHighlighted;
  return (
    <TraceCollapsible
      {...cardAttrs(toolCallId)}
      disabled={output === null}
      defaultOpen={output !== null}
    >
      <TraceCollapsibleTrigger>
        <TraceLine>
          <TraceIcon>
            <FileEditIcon />
          </TraceIcon>
          <TraceTitle>Edit {path}</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        {output?.isError ? (
          <div className="text-destructive mb-2">{output.content}</div>
        ) : (
          <CodeBlock>
            <CodeBlockHeader>{path}</CodeBlockHeader>
            <CodeBlockContent className="max-h-40">
              {oldLines.map((line, index) => (
                <CodeBlockLine key={`removed-${index}`} variant="removed">
                  {line}
                </CodeBlockLine>
              ))}
            </CodeBlockContent>
            <CodeBlockContent className="max-h-40">
              {newLines.map((line, index) => (
                <CodeBlockLine key={`added-${index}`} variant="added">
                  {line}
                </CodeBlockLine>
              ))}
            </CodeBlockContent>
          </CodeBlock>
        )}
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const BUILD_IN_TOOL_CARDS: Record<string, AgentToolCard> = {
  AskUserQuestion: AskUserQuestionCard,
  finish_step: FinishStepCard,
  web_search: WebSearchCard,
  bash: BashCard,
  read: ReadCard,
  write: WriteCard,
  edit: EditCard,
};
