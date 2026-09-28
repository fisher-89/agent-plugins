/**
 * 工具卡注册表：特化工具卡可插拔位（name 命中特化卡、miss 回退消费方
 * 默认工具对卡）。AskUserQuestion 静态卡自 explore-conversation 收编——
 * 问题与选项原样可读，答案经 composer 文本输入 + resume 续话（questionnaire
 * 接线为二期，能力 spec 留痕）。
 */

import type { AgentToolOutput } from '../../lib/agent-adapter';

/** 特化工具卡入参（output 为 null 即尚无结果；渲染 or null 回退由卡自决） */
interface AgentToolCardProps {
  toolCallId: string;
  toolName: string;
  input: unknown;
  output: AgentToolOutput | null;
}

/** 特化工具卡可插拔位类型 */
export type AgentToolCard = (props: AgentToolCardProps) => React.JSX.Element | null;

/** AskUserQuestion 静态卡的问题/选项形态（宽容解析，未知形态回退 JSON 原样） */
interface AskOption {
  label: string;
  description: string | null;
}
interface AskQuestion {
  question: string;
  options: AskOption[];
}

/** 宽松对象化：非对象一律空对象（免断言的未知形态收敛） */
function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return {};
  const record: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) record[key] = item;
  return record;
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

/** 默认注册表：AskUserQuestion → 静态卡（可插拔，消费方按 name 查找） */
export const DEFAULT_TOOL_CARDS: Record<string, AgentToolCard> = {
  AskUserQuestion: AskUserQuestionCard,
};
