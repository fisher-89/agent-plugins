/**
 * 特化工具卡注册表：`BUILD_IN_TOOL_CARDS` 按工具名装配卡片（未注册工具走通用卡）。
 * 第 8 张卡 `propose_config` 是配置提案卡（五态 + 字段级 diff + 唯一写回入口
 * 「应用」）；卡组件一律不外露——公共面留白形态归
 * `App.Frontend.Components.Agent.ToolCards`。
 */
import {
  type QuestionnaireChoiceDefinition,
  type QuestionnaireItemDefinition,
} from '@shadcn/react/questionnaire';
import { FileEditIcon, FileIcon, MessageCircleQuestionIcon, TerminalIcon } from 'lucide-react';
import { type ComponentType, useState } from 'react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  CodeBlock,
  CodeBlockContent,
  CodeBlockHeader,
  CodeBlockLine,
} from '@/components/ui/code-block';
import { useHighlightedCode } from '@/components/ui/hooks/use-highlighted-code';
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
import type { AgentToolOutput } from '@/lib/agent-adapter';
import { ipc } from '@/lib/ipc';
import { asRecord, numberOf, textOf } from '@/lib/parse';

import { FinishStepCard, WebSearchCard } from './blocks';
import { useConfigApplyActions } from './config-apply-scope';

/** 特化工具卡入参（output 为 null 即尚无结果；渲染 or null 回退由卡自决） */
interface AgentToolCardProps {
  toolCallId: string;
  toolName: string;
  state: 'input-available' | 'output-available';
  input: unknown;
  output: AgentToolOutput | null;
  /** 该轮是否仍在运行（缺席按运行中处理——无轮行的历史转录与流式未挂 `record`
   * 两情形）；只穿原语，卡层不认识轮状态词表 */
  turnRunning?: boolean;
  /** ask 应答动作面（在场且本卡为活卡时呈可交互形态；缺席即静态只读卡） */
  askAnswer?: { onAnswer: (text: string) => void };
}

/**
 * 未配对部件的运行态判据单点（四处 `TraceLine status` 与 Ask / Bash 的运行图标
 * 共用）：有结果即非运行态；无结果且轮已终结不呈运行态——未配对只说明没有结果
 * 落库，不说明正在跑。
 */
function unpairedRunning(
  output: AgentToolOutput | null,
  turnRunning: boolean | undefined,
): boolean {
  return output === null && turnRunning !== false;
}

/** 特化工具卡可插拔位类型（按组件装配，允许 memo 包装） */
export type AgentToolCard = ComponentType<AgentToolCardProps>;

/** ask_user_question 静态卡的问题/选项形态（宽容解析，未知形态回退 JSON 原样） */
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
 * - 单问形态 `{ name, question, options }`（阻塞工具 `ask_user_question` 的当前线
 *   形态；`name` 是**协议声明**的应答 map 键，缺失是历史转录，回落 `Q1`）；
 * - 历史形态 `{ questions: [{ question, options }] }`（旧工具参数面）。
 */
function parseAsk(input: unknown): AskQuestion[] | null {
  const record = asRecord(input);
  if (typeof record.question === 'string') {
    return [
      {
        name: typeof record.name === 'string' && record.name !== '' ? record.name : 'Q1',
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
 * 已应答回灌的选项回填：工具结果正文是**后端铸造**的应答信封
 * `{ status: 'answered', answers: { 问题名: 应答原文 } }`（形状单点在
 * `execute_ask`——本卡与右栏 `AskCard` 都只上行原文，不拼结构）。
 *
 * 历史转录两种形态兼容：旧扁平 map `{ 问题名: 值 }`（无 `status`）与更早的
 * 自由文本原文。非 JSON / 无 `answers` 对象时返回 `null`，由调用方按「无匹配
 * 选项」呈现原文——不抛错。
 */
function parseSubmittedAnswers(output: AgentToolOutput | null): Record<string, string> | null {
  if (output === null || output.isError) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(output.content);
  } catch {
    return null;
  }
  const envelope = asRecord(parsed);
  const body = 'status' in envelope ? asRecord(envelope.answers) : envelope;
  const answers: Record<string, string> = {};
  for (const [key, value] of Object.entries(body)) {
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
 * ask_user_question 特化卡双态（同一组件、同一卡根钩子）：
 * - 静态态（未注入动作面）：问题与选项原样可读、零交互；
 * - 活卡态（注入动作面）：选项为可点按钮 + 自由文本输入与提交；提交后本地
 *   即时禁用（防连点 / 重复应答），等待上层状态变化复位。
 */
function AskUserQuestionCard({
  input,
  output,
  turnRunning,
  askAnswer,
}: AgentToolCardProps): React.JSX.Element {
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

  // 单点判据先取值（status 与图标两处消费；本函数受 80 行上限约束）
  const running = unpairedRunning(output, turnRunning);
  // 活卡判据含「单问」：应答上行的是用户原文（一值），多问形态只存在于历史
  // 转录（当前协议一次一问、`name` 定键），故历史多问卡呈只读——不提供会静默
  // 丢弃其余问答的提交入口。
  const live = output === null && askAnswer !== undefined && !submitted && questions.length === 1;
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
    // 单问必答：上行用户原文（选项值或自由文本）。结构（信封 / 键位）由后端按
    // 工具声明铸造——本卡不拼 JSON，两入口（此卡与右栏 AskCard）载荷同形。
    const text = Object.values(answers)[0];
    if (text === undefined || text === '' || askAnswer === undefined || submitted) return;
    setSubmitted(true);
    askAnswer.onAnswer(text);
  };

  return (
    <>
      <TraceLine
        status={output ? (output.isError ? 'error' : undefined) : running ? 'running' : undefined}
      >
        <TraceIcon>{running ? <Spinner /> : <MessageCircleQuestionIcon />}</TraceIcon>
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

/** Bash 特化卡：命令作展开态标题栏、`intent` 作折叠态标题；输出折叠在下方（`intent` 缺席时折叠态仅工具名） */
function BashCard({
  toolCallId,
  input,
  output,
  turnRunning,
}: AgentToolCardProps): React.JSX.Element {
  const record = asRecord(input);
  const command = textOf(record.command);
  const intent = textOf(record.intent)?.trim() ?? '';
  return (
    <TraceCollapsible {...cardAttrs(toolCallId)}>
      <TraceCollapsibleTrigger>
        <TraceLine
          status={
            output
              ? output.isError
                ? 'error'
                : undefined
              : unpairedRunning(output, turnRunning)
                ? 'running'
                : undefined
          }
        >
          <TraceIcon>
            {unpairedRunning(output, turnRunning) ? <Spinner /> : <TerminalIcon />}
          </TraceIcon>
          <TraceTitle>{intent === '' ? 'Bash' : `Bash · ${intent}`}</TraceTitle>
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
function ReadCard({
  toolCallId,
  input,
  output,
  turnRunning,
}: AgentToolCardProps): React.JSX.Element {
  const record = asRecord(input);
  const path = textOf(record.path);
  const offset = numberOf(record.offset);
  return (
    <TraceCollapsible {...cardAttrs(toolCallId)} disabled={output === null}>
      <TraceCollapsibleTrigger>
        <TraceLine
          status={
            output
              ? output.isError
                ? 'error'
                : undefined
              : unpairedRunning(output, turnRunning)
                ? 'running'
                : undefined
          }
        >
          <TraceIcon>
            <FileIcon />
          </TraceIcon>
          <TraceTitle>Read · {`${path}${rangeSuffix(record.offset, record.limit)}`}</TraceTitle>
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
function WriteCard({
  toolCallId,
  input,
  output,
  turnRunning,
}: AgentToolCardProps): React.JSX.Element {
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
        <TraceLine
          status={
            output
              ? output.isError
                ? 'error'
                : undefined
              : unpairedRunning(output, turnRunning)
                ? 'running'
                : undefined
          }
        >
          <TraceIcon>
            {unpairedRunning(output, turnRunning) ? <Spinner /> : <FileEditIcon />}
          </TraceIcon>
          <TraceTitle>Write · {path}</TraceTitle>
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
          <TraceTitle>Edit · {path}</TraceTitle>
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

/** `propose_config` 载荷的宽容形态（后端单源产出：前端只判「有没有 candidate /
 * `changes` 是否为空」，不读配置语义、不重算差异） */
interface ConfigProposeChange {
  path: string;
  kind: string;
}

interface ConfigProposePayload {
  current: unknown;
  candidate: unknown;
  changes: ConfigProposeChange[];
  summary: string;
}

/** 载荷解析：非 `is_error` 的 `content` 必须是单条 JSON（不可解析即 `null` → 原文降级） */
function parseConfigPropose(output: AgentToolOutput | null): ConfigProposePayload | null {
  if (output === null || output.isError) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(output.content);
  } catch {
    return null;
  }
  const record = asRecord(parsed);
  if (!Object.hasOwn(record, 'candidate')) return null;
  const changes = Array.isArray(record.changes)
    ? record.changes.map((change: unknown) => {
        const item = asRecord(change);
        return { path: textOf(item.path) ?? '', kind: textOf(item.kind) ?? 'changed' };
      })
    : [];
  return {
    current: record.current,
    candidate: record.candidate ?? null,
    changes,
    summary: textOf(record.summary) ?? '',
  };
}

/** 差异行（`changed` 拆 removed + added 两行，与 `EditCard` 的双段形态同源） */
function configDiffLines(changes: readonly ConfigProposeChange[]): Array<{
  key: string;
  variant: 'added' | 'removed';
  text: string;
}> {
  return changes.flatMap((change, index) => {
    if (change.kind === 'changed') {
      return [
        { key: `${index}-removed`, variant: 'removed' as const, text: `- ${change.path}` },
        { key: `${index}-added`, variant: 'added' as const, text: `+ ${change.path}` },
      ];
    }
    const added = change.kind === 'added';
    return [
      {
        key: `${index}`,
        variant: added ? ('added' as const) : ('removed' as const),
        text: `${added ? '+' : '-'} ${change.path}`,
      },
    ];
  });
}

/** 写回会重排键序并格式化——用户在下笔前就知道整份文件会被重写 */
const CONFIG_NORMALIZE_NOTICE = '写回会规范化文件（未知子键、键序与格式会丢失 / 变化）';

/** JSON 文本 → 逐行渲染（`CodeBlockLine` 只吃行） */
function jsonLines(value: unknown): string[] {
  return JSON.stringify(value, null, 2).split(/\n/);
}

/** 只读模式的盘面块（`candidate === null`：只呈当前配置，无写回入口） */
function ConfigProposeCurrent({ current }: { current: unknown }): React.JSX.Element {
  return (
    <CodeBlock data-testid="config-propose-current">
      <CodeBlockHeader>当前盘面</CodeBlockHeader>
      <CodeBlockContent className="max-h-60">
        {jsonLines(current).map((line, index) => (
          <CodeBlockLine key={index}>{line}</CodeBlockLine>
        ))}
      </CodeBlockContent>
    </CodeBlock>
  );
}

/** 差异块：`changes` 为空即「与当前配置一致」（唯一「无差异」判据，不跨时刻比较） */
function ConfigProposeDiff({
  changes,
}: {
  changes: readonly ConfigProposeChange[];
}): React.JSX.Element {
  if (changes.length === 0) {
    return (
      <div className="text-muted-foreground text-[12px]" data-testid="config-propose-unchanged">
        与当前配置一致
      </div>
    );
  }
  return (
    <CodeBlock data-testid="config-propose-diff">
      <CodeBlockHeader>{`候选变更 ${changes.length} 处`}</CodeBlockHeader>
      <CodeBlockContent className="max-h-60">
        {configDiffLines(changes).map((line) => (
          <CodeBlockLine key={line.key} variant={line.variant}>
            {line.text}
          </CodeBlockLine>
        ))}
      </CodeBlockContent>
    </CodeBlock>
  );
}

/**
 * 写回动作条：唯一写回入口（「应用」→ `workspace_config_apply` → 页面重取）。
 * `actions === null`（不在配置页）时按钮禁用并说明；失败 inline 呈现且保持可重试
 * （候选不被丢弃）。
 */
function ConfigProposeApplyBar({
  candidate,
  changesCount,
}: {
  candidate: unknown;
  changesCount: number;
}): React.JSX.Element {
  const actions = useConfigApplyActions();
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applied, setApplied] = useState(false);
  const applicable = actions !== null && changesCount > 0;

  const apply = (): void => {
    if (actions === null) return;
    setApplying(true);
    setApplyError(null);
    ipc
      .workspaceConfigApply(actions.root, candidate)
      .then(() => {
        setApplied(true);
        actions.onApplied();
      })
      .catch((cause: unknown) => {
        setApplyError(String(cause));
      })
      .finally(() => {
        setApplying(false);
      });
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button
        size="xs"
        disabled={!applicable || applying}
        data-testid="config-propose-apply"
        onClick={apply}
      >
        {applying ? '应用中…' : '应用'}
      </Button>
      {actions === null && (
        <span className="text-muted-foreground text-[12px]">应用只在配置页可用</span>
      )}
      {applied && (
        <span className="text-muted-foreground text-[12px]" data-testid="config-propose-applied">
          已应用
        </span>
      )}
      {applyError !== null && (
        <span
          className="text-destructive text-[12px] break-all"
          data-testid="config-propose-apply-error"
        >
          {`应用失败：${applyError}`}
        </span>
      )}
    </div>
  );
}

/** 可应用态的提案体：规范化明示 → 差异 → 写回动作条 */
function ConfigProposeCandidate({ payload }: { payload: ConfigProposePayload }): React.JSX.Element {
  return (
    <>
      <div className="text-[12px]">{CONFIG_NORMALIZE_NOTICE}</div>
      <ConfigProposeDiff changes={payload.changes} />
      <ConfigProposeApplyBar candidate={payload.candidate} changesCount={payload.changes.length} />
    </>
  );
}

/**
 * 配置提案卡（`propose_config`，注册表第 8 键）：五态（进行中 / 失败 / 可应用 /
 * 与当前一致 / 只读盘面 / 不可解析原文降级），差异与摘要全取载荷，唯一决策是
 * 「有没有 candidate、changes 是否为空」。
 */
function ConfigProposeCard({
  toolCallId,
  output,
  turnRunning,
}: AgentToolCardProps): React.JSX.Element {
  const payload = parseConfigPropose(output);
  return (
    <TraceCollapsible
      {...cardAttrs(toolCallId)}
      disabled={output === null}
      defaultOpen={output !== null}
    >
      <TraceCollapsibleTrigger>
        <TraceLine
          status={
            output
              ? output.isError
                ? 'error'
                : undefined
              : unpairedRunning(output, turnRunning)
                ? 'running'
                : undefined
          }
        >
          <TraceIcon>
            {unpairedRunning(output, turnRunning) ? <Spinner /> : <FileEditIcon />}
          </TraceIcon>
          <TraceTitle>Config proposal</TraceTitle>
        </TraceLine>
      </TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        {output === null && (
          <div className="text-muted-foreground" data-testid="config-propose-running">
            正在准备提案…
          </div>
        )}
        {output?.isError === true && (
          <div
            className="text-destructive mb-2 whitespace-pre-wrap"
            data-testid="config-propose-error"
          >
            {output.content}
          </div>
        )}
        {output !== null && !output.isError && payload === null && (
          <pre
            className="overflow-auto text-[12px] whitespace-pre-wrap"
            data-testid="config-propose-raw"
          >
            {output.content}
          </pre>
        )}
        {payload !== null && (
          <div className="flex flex-col gap-2">
            {payload.summary !== '' && (
              <div
                className="text-muted-foreground text-[12px]"
                data-testid="config-propose-summary"
              >
                {payload.summary}
              </div>
            )}
            {payload.candidate === null ? (
              <ConfigProposeCurrent current={payload.current} />
            ) : (
              <ConfigProposeCandidate payload={payload} />
            )}
          </div>
        )}
      </TraceCollapsibleContent>
    </TraceCollapsible>
  );
}

export const BUILD_IN_TOOL_CARDS: Record<string, AgentToolCard> = {
  /** @deprecated 兼容旧版工具名 */
  AskUserQuestion: AskUserQuestionCard,
  ask_user_question: AskUserQuestionCard,
  finish_step: FinishStepCard,
  web_search: WebSearchCard,
  bash: BashCard,
  read: ReadCard,
  write: WriteCard,
  edit: EditCard,
  propose_config: ConfigProposeCard,
};
