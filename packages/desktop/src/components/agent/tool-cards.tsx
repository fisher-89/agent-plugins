import { FileEditIcon, FileIcon, TerminalIcon } from 'lucide-react';
import { type ComponentType, useState } from 'react';

import { Button } from '@/components/ui/button';
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
  /** ask 应答动作面（在场且本卡为活卡时呈可交互形态；缺席即静态只读卡） */
  askAnswer?: { onAnswer: (text: string) => void };
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

/** 选项归一：新工具形态的元素是裸字符串（`description` 缺席），历史 SDK 形态
 * 是 `{ label, description }` 对象——两形态由元素形态直接判别，无需外部标记。 */
function parseOptions(options: unknown[]): AskOption[] {
  return options.map((option: unknown) => {
    if (typeof option === 'string') return { label: option, description: null };
    const item = asRecord(option);
    return {
      label: typeof item.label === 'string' ? item.label : '',
      description: typeof item.description === 'string' ? item.description : null,
    };
  });
}

/**
 * 宽松解析 ask 输入（双形态归一，皆不匹配即 null → 卡内回退 JSON 展示）：
 * - 单问形态 `{ question, options }`（阻塞工具 `AskUserQuestion` 的当前线形态）；
 * - 历史形态 `{ questions: [{ question, options }] }`（旧工具参数面）。
 */
function parseAsk(input: unknown): AskQuestion[] | null {
  const record = asRecord(input);
  if (typeof record.question === 'string') {
    return [
      {
        question: record.question,
        options: Array.isArray(record.options) ? parseOptions(record.options) : [],
      },
    ];
  }
  const questions: unknown = record.questions;
  if (!Array.isArray(questions)) return null;
  return questions.map((q: unknown) => {
    const item = asRecord(q);
    return {
      question: typeof item.question === 'string' ? item.question : '',
      options: Array.isArray(item.options) ? parseOptions(item.options) : [],
    };
  });
}

/** 单问题：题干 + 选项清单（活卡态选项为可点按钮，静态态为只读行） */
function AskQuestionView({
  item,
  onPick,
}: {
  item: AskQuestion;
  onPick?: (label: string) => void;
}): React.JSX.Element {
  return (
    <div className="mb-2 last:mb-0">
      <div className="text-sm" data-testid="ask-question">
        {item.question}
      </div>
      <ul className="m-0 mt-1 list-none p-0">
        {item.options.map((option, optionIndex) =>
          onPick === undefined ? (
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
          ) : (
            <li key={optionIndex} className="mb-1 last:mb-0">
              <Button
                variant="outline"
                size="sm"
                className="w-full justify-start"
                data-testid="ask-answer-option"
                onClick={() => onPick(option.label)}
              >
                {option.label}
                {option.description !== null && (
                  <span className="ml-1.5 text-xs text-muted-foreground">{option.description}</span>
                )}
              </Button>
            </li>
          ),
        )}
      </ul>
    </div>
  );
}

/**
 * AskUserQuestion 特化卡双态（同一组件、同一卡根钩子）：
 * - 静态态（未注入动作面）：问题与选项原样可读、零交互；
 * - 活卡态（注入动作面）：选项为可点按钮 + 自由文本输入与提交；提交后本地
 *   即时禁用（防连点 / 重复应答），等待上层状态变化复位。
 */
function AskUserQuestionCard({ input, askAnswer }: AgentToolCardProps): React.JSX.Element {
  const questions = parseAsk(input);
  const [submitted, setSubmitted] = useState(false);
  const [text, setText] = useState('');
  const live = askAnswer !== undefined && !submitted;
  const submit = (value: string) => {
    const trimmed = value.trim();
    if (trimmed === '' || askAnswer === undefined || submitted) return;
    setSubmitted(true);
    setText('');
    askAnswer.onAnswer(trimmed);
  };
  return (
    <div
      className="my-1 rounded-md border border-border bg-muted px-3 py-2.5"
      data-testid="ask-question-card"
    >
      <div className="mb-1 text-xs font-medium text-muted-foreground">
        {live
          ? '向用户提问（AskUserQuestion）· 选择一个选项或自行作答'
          : '向用户提问（AskUserQuestion）'}
      </div>
      {questions === null ? (
        <pre className="m-0 overflow-x-auto rounded bg-background px-2 py-1.5 text-xs">
          {JSON.stringify(input, null, 2)}
        </pre>
      ) : (
        questions.map((item, index) => (
          <AskQuestionView key={index} item={item} onPick={live ? submit : undefined} />
        ))
      )}
      {live && (
        <div className="mt-1 flex items-center gap-1.5">
          <input
            className="min-w-0 flex-1 rounded border border-border bg-background px-2 py-1 text-sm"
            data-testid="ask-answer-input"
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') submit(text);
            }}
          />
          <Button
            size="sm"
            data-testid="ask-answer-submit"
            disabled={submitted || text.trim() === ''}
            onClick={() => submit(text)}
          >
            应答
          </Button>
        </div>
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
