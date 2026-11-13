import {
  type QuestionnaireChoiceDefinition,
  type QuestionnaireItemDefinition,
} from '@shadcn/react/questionnaire';
import { FileEditIcon, FileIcon, MessageCircleQuestionIcon, TerminalIcon } from 'lucide-react';
import { type ComponentType, useState } from 'react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Card, CardContent } from '@/components/ui/card';
import {
  CodeBlock,
  CodeBlockContent,
  CodeBlockHeader,
  CodeBlockLine,
} from '@/components/ui/code-block';
import {
  Questionnaire,
  QuestionnaireActions,
  QuestionnaireChoice,
  QuestionnaireChoiceDescription,
  QuestionnaireChoices,
  QuestionnaireError,
  QuestionnaireInput,
  QuestionnaireItem,
  QuestionnaireNext,
  QuestionnairePrevious,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from '@/components/ui/questionnaire';
import { Spinner } from '@/components/ui/spinner';
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
interface AskOption extends QuestionnaireChoiceDefinition {
  description: string | null;
}
interface AskQuestion extends QuestionnaireItemDefinition {
  question: string;
  choices: AskOption[];
}

/** 选项归一：新工具形态的元素是裸字符串（`description` 缺席），历史 SDK 形态
 * 是 `{ label, description }` 对象——两形态由元素形态直接判别，无需外部标记。 */
function parseOptions(options: unknown[]): AskOption[] {
  return options.map((option: unknown) => {
    if (typeof option === 'string') return { value: option, description: null };
    const item = asRecord(option);
    return {
      value: typeof item.label === 'string' ? item.label : '',
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
        name: 'Q1',
        question: record.question,
        choices: Array.isArray(record.options) ? parseOptions(record.options) : [],
      },
    ];
  }
  const questions: unknown = record.questions;
  if (!Array.isArray(questions)) return null;
  return questions.map((q: unknown, index) => {
    const item = asRecord(q);
    return {
      name: `Q${index + 1}`,
      question: typeof item.question === 'string' ? item.question : '',
      choices: Array.isArray(item.options) ? parseOptions(item.options) : [],
    };
  });
}

/**
 * 已应答回灌的选项回填：工具结果正文是本卡提交时上行的 JSON map（`{ 问题名: 选项值 }`）。
 * 历史转录里的结果正文是自由文本原文（旧线应答）或错误文案，非 JSON 对象——此时
 * 无选项可回填，返回 `null` 由调用方按「无匹配选项」呈现原文。
 */
function parseSubmittedAnswers(output: AgentToolOutput | null): Record<string, string> | null {
  if (output === null || output.isError) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(output.content);
  } catch {
    return null;
  }
  const record = asRecord(parsed);
  const answers: Record<string, string> = {};
  for (const [key, value] of Object.entries(record)) {
    if (typeof value !== 'string') return null;
    answers[key] = value;
  }
  return Object.keys(answers).length === 0 ? null : answers;
}

/** 单问的选项区（选项行 + 已应答原文回显 + 活卡自由文本输入） */
function AskChoicesView({
  item,
  answered,
  live,
}: {
  item: AskQuestion;
  /** 该问已回灌的应答值（选项值或自由文本；`null` = 尚未应答） */
  answered: string | null;
  live: boolean;
}): React.JSX.Element {
  return (
    <QuestionnaireChoices>
      {item.choices.map((option) => (
        <QuestionnaireChoice
          data-testid="ask-answer-option"
          className="py-2 min-h-10"
          key={option.value}
          value={option.value}
          disabled={answered !== null && answered !== option.value}
          defaultChecked={answered === option.value}
        >
          {option.value}
          <QuestionnaireChoiceDescription>{option.description}</QuestionnaireChoiceDescription>
        </QuestionnaireChoice>
      ))}
      {answered !== null && item.choices.every((option) => option.value !== answered) && (
        <div className="rounded-lg border border-primary/40 bg-muted px-3 py-2 text-start text-sm">
          {answered}
        </div>
      )}
      {live && <QuestionnaireInput data-testid="ask-answer-input" placeholder="或自行输入回复" />}
    </QuestionnaireChoices>
  );
}

/**
 * AskUserQuestion 特化卡双态（同一组件、同一卡根钩子）：
 * - 静态态（未注入动作面）：问题与选项原样可读、零交互；
 * - 活卡态（注入动作面）：选项为可点按钮 + 自由文本输入与提交；提交后本地
 *   即时禁用（防连点 / 重复应答），等待上层状态变化复位。
 */
function AskUserQuestionCard({ input, output, askAnswer }: AgentToolCardProps): React.JSX.Element {
  const questions = parseAsk(input);
  const [submitted, setSubmitted] = useState(false);
  if (questions === null) {
    return (
      <Alert variant="destructive">
        <AlertTitle>无法解析问题，原始输入：</AlertTitle>
        <AlertDescription>{JSON.stringify(input, null, 2)}</AlertDescription>
      </Alert>
    );
  }

  const live = output === null && askAnswer !== undefined && !submitted;
  const submittedForm = parseSubmittedAnswers(output);
  // Root 的 items 与逐项渲染的 Choice 必须同源（库按值对账，不一致会逐渲染告警）
  const items: QuestionnaireItemDefinition[] = questions.map((item) => ({
    name: item.name,
    choices: item.choices.map((option) => ({
      value: option.value,
      disabled: submittedForm !== null && submittedForm[item.name] !== option.value,
    })),
  }));
  const handleSubmit = (event: React.SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    const answers: Record<string, string> = {};
    new FormData(event.currentTarget).forEach((value, key) => {
      answers[key] = value instanceof File ? value.name : value;
    });
    const trimmed = JSON.stringify(answers);
    if (trimmed === '{}' || askAnswer === undefined || submitted) return;
    setSubmitted(true);
    askAnswer.onAnswer(trimmed);
  };

  return (
    <>
      <TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}>
        <TraceIcon>{output === null ? <Spinner /> : <MessageCircleQuestionIcon />}</TraceIcon>
        <TraceTitle>提问</TraceTitle>
      </TraceLine>
      <Card size="sm">
        <CardContent>
          <Questionnaire data-testid="ask-question-card" items={items} onSubmit={handleSubmit}>
            {questions.map((item) => (
              <QuestionnaireItem data-testid="ask-question" name={item.name} key={item.name}>
                <QuestionnaireTitle className="text-sm">{item.question}</QuestionnaireTitle>
                <AskChoicesView
                  item={item}
                  answered={submittedForm?.[item.name] ?? null}
                  live={live}
                />
                {live && <QuestionnaireError />}
              </QuestionnaireItem>
            ))}
            {live && (
              <QuestionnaireActions>
                <QuestionnairePrevious>上一个</QuestionnairePrevious>
                <QuestionnaireNext>下一个</QuestionnaireNext>
                <QuestionnaireSubmit data-testid="ask-answer-submit">提交</QuestionnaireSubmit>
              </QuestionnaireActions>
            )}
          </Questionnaire>
        </CardContent>
      </Card>
    </>
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
