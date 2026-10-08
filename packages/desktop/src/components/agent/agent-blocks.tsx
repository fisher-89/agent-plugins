/**
 * 双透镜共享底层块渲染件：文本 markdown / 思考折叠 / 工具对卡 / result
 * 汇总卡 / 可复制值 / record 静默状态行。自 explore-conversation 与
 * agent-event-timeline 收编（既有 block-* / event-* / result-* /
 * copy-value data-testid 保留，支撑迁移用例基线）。
 */

import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import type { AgentToolOutput, AgentToolPart, AgentUIMessage } from '../../lib/agent-adapter';
import type { AgentRunStatus, TurnSummary } from '../../types/dto';

/** 消息部件 → tool 部件收窄（`tool-<name>` 命名，dynamic-tool 不在信封内） */
export function isToolPart(part: AgentUIMessage['parts'][number]): part is AgentToolPart {
  return part.type.startsWith('tool-') && part.type !== 'dynamic-tool';
}

/** 可复制值：原文 + 复制按钮（result 汇总卡 sessionId 一处语义） */
function CopyValue({ value }: { value: string }): React.JSX.Element {
  return (
    <span className="inline-flex items-center gap-1">
      <code className="break-all rounded bg-muted px-1 py-0.5 text-xs">{value}</code>
      <button
        type="button"
        className="cursor-pointer border-0 bg-transparent p-0 text-xs text-muted-foreground underline hover:text-foreground"
        data-testid="copy-value"
        onClick={() => {
          void navigator.clipboard.writeText(value);
        }}
      >
        复制
      </button>
    </span>
  );
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

/** 汇总卡载荷（run-result data 部件的必填子集，双透镜共用） */
export interface AgentRunResultCardData {
  subtype: string;
  isError: boolean;
  numTurns: number | null;
  costUsd: number | null;
  durationMs: number | null;
  sessionId: string | null;
  /** 轮末权威 usage（透传不解释；token 取数见 [`usageTokenCounts`]） */
  usage?: unknown;
}

/** usage 线格式取数：CLI result 与 SDK rig Usage 的 token 字段同为 snake_case
 *  `input_tokens` / `output_tokens`（缓存类字段两侧命名不一，不并入）；usage
 *  缺省或任一字段非数 → null（token 项不呈现，维持既有 stat 行为） */
function usageTokenCounts(usage: unknown): { input: number; output: number } | null {
  if (typeof usage !== 'object' || usage === null) return null;
  const input = (usage as { input_tokens?: unknown }).input_tokens;
  const output = (usage as { output_tokens?: unknown }).output_tokens;
  if (typeof input !== 'number' || typeof output !== 'number') return null;
  return { input, output };
}

/** run 结束汇总卡：轮数 / 成本 / 时长 / tokens / session（可读、session 可复制） */
export function ResultCard({ data }: { data: AgentRunResultCardData }): React.JSX.Element {
  const tokens = usageTokenCounts(data.usage);
  return (
    <div
      className={`my-2 rounded-md border px-3 py-2.5 text-sm ${data.isError ? 'border-fail bg-fail-bg text-fail' : 'border-border bg-muted'}`}
      data-testid="event-result"
      data-is-error={data.isError}
    >
      <div className="mb-1 font-semibold">
        run 结束（{data.subtype}）{data.isError ? '· 失败' : '· 成功'}
      </div>
      <div className="flex flex-wrap gap-x-5 gap-y-1 text-xs" data-testid="result-summary">
        <span>
          轮数：<strong data-testid="result-num-turns">{data.numTurns ?? '—'}</strong>
        </span>
        <span>
          成本：
          <strong data-testid="result-cost">
            {data.costUsd !== null ? `$${data.costUsd}` : '—'}
          </strong>
        </span>
        <span>
          时长：
          <strong data-testid="result-duration">
            {data.durationMs !== null ? `${data.durationMs}ms` : '—'}
          </strong>
        </span>
        {tokens !== null && (
          <span>
            tokens：
            <strong data-testid="result-tokens">
              入 {tokens.input} / 出 {tokens.output}
            </strong>
          </span>
        )}
        <span className="flex items-center gap-1">
          session：
          {data.sessionId !== null ? (
            <CopyValue value={data.sessionId} />
          ) : (
            <strong data-testid="result-session">—</strong>
          )}
        </span>
      </div>
    </div>
  );
}

/** status 受控终态的呈现后缀（stopped 与 failed 语义区分） */
function statusSuffix(status: AgentRunStatus): string {
  if (status === 'running') return '';
  if (status === 'failed') return '· 失败';
  if (status === 'stopped') return '· 已停止';
  return '· 成功';
}

/** record 静默状态行：终态轮行的对话内低调呈现（不抢对话流） */
export function RunRecordRow({ record }: { record: TurnSummary }): React.JSX.Element {
  return (
    <div
      className="my-1.5 flex items-center gap-2 text-xs text-muted-foreground"
      data-testid="event-run-record"
      data-status={record.status}
    >
      <span>
        轮结束（{record.status}）{statusSuffix(record.status)}
      </span>
      {record.error !== null && (
        <span className="break-all text-fail" data-testid="run-record-error">
          {record.error}
        </span>
      )}
    </div>
  );
}
